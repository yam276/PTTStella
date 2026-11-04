//! Explicit native-editor import/replacement. No path ever publishes a post.
use crate::{
    drafts::{MAX_BODY, Scope},
    editor_color,
    screen::Frame,
};
use serde::{Deserialize, Serialize};
#[cfg(any(test, feature = "desktop"))]
#[doc(hidden)]
pub mod fixture;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub scope: Scope,
    pub title: String,
    pub body: String,
}
#[derive(Clone)]
pub struct Replacement {
    pub baseline: Snapshot,
    pub title: String,
    pub body: String,
}
pub enum Action {
    Send(Vec<u8>),
    Wait,
    Done(Snapshot),
}
#[derive(Clone, Copy)]
enum Stage {
    Start,
    Menu,
    Title,
    Eof,
    Row,
    Restore,
    WriteMenu,
    WriteTitle,
    Clear,
    Empty,
    Written,
}
pub struct Flow {
    scope: Scope,
    stage: Stage,
    revision: u64,
    preview: bool,
    insert: bool,
    title: String,
    lines: Vec<String>,
    count: usize,
    deleting: usize,
    replacement: Option<Replacement>,
    changed: bool,
    written_position: Option<(usize, usize)>,
}
impl Flow {
    pub fn new(
        frame: &Frame,
        scope: Scope,
        replacement: Option<Replacement>,
    ) -> Result<Self, String> {
        let state = editor_color::status(frame).ok_or(crate::msg!("draft-template-unavailable"))?;
        if !scope.valid() {
            return Err(crate::msg!("draft-template-unavailable"));
        }
        let mut written_position = None;
        if let Some(r) = &replacement {
            if !r.baseline.scope.same(&scope) {
                return Err(crate::msg!("draft-transfer-stopped"));
            }
            crate::drafts::transfer(&r.body, scope.site.encoding)?;
            let title = scope
                .site
                .encoding
                .encode(&r.title)
                .map_err(|e| e.to_string())?;
            if title.is_empty() || title.len() > 60 || r.title.chars().any(char::is_control) {
                return Err(crate::msg!("draft-title-invalid"));
            }
            // Use the production terminal core for the final encoded column,
            // including tabs, Chinese and SGR. A partial write is not completion.
            let last = r.body.rsplit('\n').next().unwrap_or_default();
            let source = if state.preview {
                last.to_owned()
            } else {
                last.replace('\x1b', "*")
            };
            let mut screen = crate::screen::Screen::new(512, 1).map_err(str::to_owned)?;
            screen.set_encoding(scope.site.encoding);
            screen.feed(
                &scope
                    .site
                    .encoding
                    .encode(&source)
                    .map_err(|e| e.to_string())?,
            );
            written_position = Some((r.body.split('\n').count() - 1, screen.frame().cursor.0));
        }
        Ok(Self {
            scope,
            stage: Stage::Start,
            revision: frame.revision,
            preview: state.preview,
            insert: state.insert,
            title: String::new(),
            lines: Vec::new(),
            count: 0,
            deleting: 0,
            replacement,
            changed: false,
            written_position,
        })
    }
    pub fn changed(&self) -> bool {
        self.changed
    }
    fn send(&mut self, frame: &Frame, stage: Stage, bytes: Vec<u8>) -> Result<Action, String> {
        self.revision = frame.revision;
        self.stage = stage;
        Ok(Action::Send(bytes))
    }
    fn raw_start(&self, frame: &Frame) -> bool {
        editor_color::status(frame)
            .is_some_and(|s| s.line == 0 && s.col == 0 && !s.preview && s.insert == self.insert)
    }
    fn menu(frame: &Frame) -> bool {
        crate::text::rows(frame).is_some_and(|r| {
            !crate::navigation::article_editor(frame)
                && frame.cursor.1 == 2
                && r[0].text.contains("檔案處理")
                && r[1].text.contains("(T)改標題")
                && r[1].text.contains("(E)繼續")
                && r[2].text.contains("確定要儲存檔案嗎？")
        })
    }
    fn title(frame: &Frame) -> Option<String> {
        let rows = crate::text::rows(frame)?;
        if crate::navigation::article_editor(frame)
            || frame.cursor.1 != 4
            || !rows[3].mapped()
            || !rows[4].mapped()
            || !rows[4].text.starts_with("新標題：")
        {
            return None;
        }
        let title = rows[3].text.strip_prefix("舊標題：")?.trim_end().to_owned();
        (!title.is_empty() && title.len() <= 256).then_some(title)
    }
    pub fn next(&mut self, frame: &Frame) -> Result<Action, String> {
        if !frame.control_complete
            || frame.revision == self.revision && !matches!(self.stage, Stage::Start)
        {
            return Ok(Action::Wait);
        }
        match self.stage {
            Stage::Start => self.send(frame, Stage::Menu, vec![24]),
            Stage::Menu | Stage::WriteMenu => {
                if !Self::menu(frame) {
                    return Err(crate::msg!("draft-template-unavailable"));
                }
                self.send(
                    frame,
                    if matches!(self.stage, Stage::Menu) {
                        Stage::Title
                    } else {
                        Stage::WriteTitle
                    },
                    b"t\r".to_vec(),
                )
            }
            Stage::Title => {
                self.title = Self::title(frame).ok_or(crate::msg!("draft-template-unavailable"))?;
                self.send(frame, Stage::Eof, {
                    let mut b = vec![13, 27, b'u'];
                    if self.preview {
                        b.push(22);
                    }
                    b.extend([20, 12]);
                    b
                })
            }
            Stage::Eof => {
                let s = editor_color::status(frame)
                    .filter(|s| !s.preview && s.col == 0)
                    .ok_or(crate::msg!("draft-template-unavailable"))?;
                self.count = s.line + 1;
                if self.count > 4096 {
                    return Err(crate::msg!("draft-limit"));
                }
                self.send(frame, Stage::Row, vec![29, 5, 12])
            }
            Stage::Row => {
                let s = editor_color::status(frame)
                    .filter(|s| s.line == self.lines.len())
                    .ok_or(crate::msg!("draft-template-unavailable"))?;
                let _ = s;
                self.lines.push(editor_color::source_row(frame)?);
                if self.lines.iter().map(String::len).sum::<usize>() + self.lines.len() > MAX_BODY {
                    return Err(crate::msg!("draft-limit"));
                }
                if self.lines.len() < self.count {
                    self.send(frame, Stage::Row, b"\x01\x1b[B\x05\x0c".to_vec())
                } else {
                    let captured = Snapshot {
                        scope: self.scope.clone(),
                        title: self.title.clone(),
                        body: self.lines.join("\n"),
                    };
                    if let Some(r) = &self.replacement {
                        if captured != r.baseline {
                            return Err(crate::msg!("draft-template-changed"));
                        }
                        self.send(frame, Stage::WriteMenu, vec![24])
                    } else {
                        let mut b = vec![29];
                        if self.preview {
                            b.push(22);
                        }
                        b.push(12);
                        self.send(frame, Stage::Restore, b)
                    }
                }
            }
            Stage::Restore => {
                editor_color::status(frame)
                    .filter(|s| s.line == 0 && s.col == 0 && s.preview == self.preview)
                    .ok_or(crate::msg!("draft-template-unavailable"))?;
                Ok(Action::Done(Snapshot {
                    scope: self.scope.clone(),
                    title: self.title.clone(),
                    body: self.lines.join("\n"),
                }))
            }
            Stage::WriteTitle => {
                if Self::title(frame).as_deref() != Some(&self.title) {
                    return Err(crate::msg!("draft-template-changed"));
                }
                let r = self
                    .replacement
                    .as_ref()
                    .ok_or(crate::msg!("draft-transfer-stopped"))?;
                let mut b = vec![25];
                b.extend(
                    self.scope
                        .site
                        .encoding
                        .encode(&r.title)
                        .map_err(|e| e.to_string())?,
                );
                b.extend([13, 29, 12]);
                self.changed = true;
                self.send(frame, Stage::Clear, b)
            }
            Stage::Clear => {
                if !self.raw_start(frame) {
                    // Returning from the title prompt can redraw the previous
                    // cursor before the queued document-Home is processed.
                    return Ok(Action::Wait);
                }
                let n = (self.count - self.deleting).min(64);
                self.deleting += n;
                let mut b = vec![25; n];
                b.push(12);
                self.send(
                    frame,
                    if self.deleting == self.count {
                        Stage::Empty
                    } else {
                        Stage::Clear
                    },
                    b,
                )
            }
            Stage::Empty => {
                if !self.raw_start(frame) || !crate::drafts::empty_editor(frame) {
                    // Ctrl+Y batches may yield several complete redraws. None
                    // permits writing until the whole template is gone. The
                    // connection owner retains its original per-step deadline.
                    return Ok(Action::Wait);
                }
                let r = self
                    .replacement
                    .as_ref()
                    .ok_or(crate::msg!("draft-transfer-stopped"))?;
                let mut b = Vec::new();
                if !self.insert {
                    b.push(15);
                }
                b.extend(crate::drafts::transfer(&r.body, self.scope.site.encoding)?);
                if !self.insert {
                    b.push(15);
                }
                if self.preview {
                    b.push(22);
                }
                b.push(12);
                self.send(frame, Stage::Written, b)
            }
            Stage::Written => {
                if editor_color::status(frame).is_none_or(|s| {
                    Some((s.line, s.col)) != self.written_position
                        || s.preview != self.preview
                        || s.insert != self.insert
                }) {
                    return Ok(Action::Wait);
                }
                let r = self
                    .replacement
                    .as_ref()
                    .ok_or(crate::msg!("draft-transfer-stopped"))?;
                Ok(Action::Done(Snapshot {
                    scope: self.scope.clone(),
                    title: r.title.clone(),
                    body: r.body.clone(),
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope() -> Scope {
        Scope {
            site: Default::default(),
            account: "StellaTest".into(),
            board: crate::board_navigation::BoardName::parse("Stella").unwrap(),
        }
    }
    fn run(n: &mut fixture::Native, r: Option<Replacement>) -> Result<Snapshot, String> {
        let first = n.frame();
        let mut f = Flow::new(&first, scope(), r)?;
        for _ in 0..20000 {
            match f.next(&n.frame())? {
                Action::Send(b) => n.accept(&b),
                Action::Wait => {}
                Action::Done(s) => return Ok(s),
            }
        }
        panic!("flow did not terminate")
    }
    #[test]
    fn reads_offscreen_chinese_ansi_and_trailing_empty_line_without_mutating_template() {
        let lines = (0..31)
            .map(|i| format!("範本第{i}行\x1b[32m文字\x1b[m"))
            .chain([String::new()])
            .collect::<Vec<_>>();
        let mut n = fixture::Native::new(
            &lines.iter().map(String::as_str).collect::<Vec<_>>(),
            "[分類] 測試標題",
            true,
        );
        let original = n.editor.lines.clone();
        let captured = run(&mut n, None).unwrap();
        assert_eq!(captured.body, lines.join("\n"));
        assert_eq!(n.editor.lines, original);
        assert!(n.editor.preview);
        assert_eq!(n.menu_count, 1);
    }
    #[test]
    fn replacement_checks_whole_template_changes_title_and_body_without_publishing() {
        let mut n = fixture::Native::new(
            &["預設中文", "\x1b[32m必填欄位\x1b[m", ""],
            "[問卦] 原標題",
            false,
        );
        let baseline = run(&mut n, None).unwrap();
        let mut r = Replacement {
            baseline: baseline.clone(),
            title: "[分類] 中文新標題".into(),
            body: "完成中文\n\x1b[1;31m紅色\x1b[m\n".into(),
        };
        n.editor.lines[1] = crate::encoding::encode("已變更").unwrap();
        assert!(run(&mut n, Some(r.clone())).is_err());
        assert_eq!(n.title, baseline.title);
        n.editor.lines[1] = crate::encoding::encode("\x1b[32m必填欄位\x1b[m").unwrap();
        let out = run(&mut n, Some(r.clone())).unwrap();
        assert_eq!(out.body, r.body);
        assert_eq!(n.title, r.title);
        assert_eq!(
            n.editor.lines,
            r.body
                .split('\n')
                .map(|s| crate::encoding::encode(s).unwrap())
                .collect::<Vec<_>>()
        );
        assert!(!n.editor.preview);
        r.body = "malicious\x18".into();
        assert!(Flow::new(&n.frame(), scope(), Some(r)).is_err());
    }
    #[test]
    fn replacement_waits_for_title_return_and_each_delete_batch_to_finish() {
        let mut native = fixture::Native::new(&["原範本", "第二行", "第三行"], "原標題", true);
        let baseline = run(&mut native, None).unwrap();
        let replacement = Replacement {
            baseline,
            title: "新標題".into(),
            body: "中文\n\x1b[1;31m紅色\x1b[m".into(),
        };
        let mut flow = Flow::new(&native.frame(), scope(), Some(replacement.clone())).unwrap();
        let mut title_waited = false;
        let mut deletion_waited = false;
        let mut writing_waited = false;
        for _ in 0..100 {
            match flow.next(&native.frame()).unwrap() {
                Action::Send(bytes) => {
                    if matches!(flow.stage, Stage::Clear) && !title_waited {
                        // Native editor returns at the prior cursor first; Ctrl+]
                        // is processed later. Both are complete terminal frames.
                        let split = bytes.iter().position(|b| *b == 13).unwrap() + 1;
                        native.accept(&bytes[..split]);
                        assert!(matches!(flow.next(&native.frame()).unwrap(), Action::Wait));
                        native.accept(&bytes[split..]);
                        title_waited = true;
                    } else if matches!(flow.stage, Stage::Empty) && !deletion_waited {
                        // A redraw after only one Ctrl+Y is not a changed post.
                        assert_eq!(bytes, [25, 25, 25, 12]);
                        native.accept(&bytes[..1]);
                        assert_eq!(native.editor.lines.len(), 2);
                        assert!(matches!(flow.next(&native.frame()).unwrap(), Action::Wait));
                        native.accept(&bytes[1..]);
                        deletion_waited = true;
                    } else if matches!(flow.stage, Stage::Written) && !writing_waited {
                        native.accept(&bytes[..2]);
                        assert!(matches!(flow.next(&native.frame()).unwrap(), Action::Wait));
                        native.accept(&bytes[2..]);
                        writing_waited = true;
                    } else {
                        native.accept(&bytes);
                    }
                }
                Action::Wait => panic!("fixture supplied all remaining response bytes"),
                Action::Done(snapshot) => {
                    assert!(title_waited && deletion_waited && writing_waited);
                    assert_eq!(snapshot.body, replacement.body);
                    assert_eq!(native.title, "新標題");
                    assert_eq!(
                        native.editor.lines,
                        replacement
                            .body
                            .split('\n')
                            .map(|line| crate::encoding::encode(line).unwrap())
                            .collect::<Vec<_>>()
                    );
                    return;
                }
            }
        }
        panic!("replacement did not terminate");
    }

    #[test]
    fn transfer_restores_both_modes_and_waits_for_the_final_chinese_ansi_column() {
        for preview in [false, true] {
            for insert in [false, true] {
                for body in ["中文\n\x1b[1;31m紅色\x1b[m", "中文\n"] {
                    let mut native = fixture::Native::new(&["原範本"], "原標題", preview);
                    native.editor.insert = insert;
                    let baseline = run(&mut native, None).unwrap();
                    let replacement = Replacement {
                        baseline,
                        title: "新標題".into(),
                        body: body.into(),
                    };
                    assert_eq!(run(&mut native, Some(replacement)).unwrap().body, body);
                    assert_eq!(
                        (native.editor.preview, native.editor.insert),
                        (preview, insert)
                    );
                }
            }
        }
    }

    #[test]
    fn unknown_menu_and_incomplete_or_long_rows_are_not_guessed() {
        let mut n = fixture::Native::new(&[&"x".repeat(90)], "標題", false);
        assert!(run(&mut n, None).is_err());
        let frame = fixture::Native::new(&["safe"], "標題", false).frame();
        let mut f = Flow::new(&frame, scope(), None).unwrap();
        assert!(matches!(f.next(&frame).unwrap(), Action::Send(_)));
        let mut wrong = frame.clone();
        wrong.revision += 1;
        assert!(f.next(&wrong).is_err());
    }
}
