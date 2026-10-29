//! Guarded PTT editor colouring. Consumes mapped frames, never session bytes.
//! Existing SGRs stay in the document; a new colour overrides each SGR inside
//! the selection. No selected text is deleted or re-encoded.
use crate::{
    navigation,
    screen::{Frame, Style},
    selection::{self, Selection, Span},
    text,
};
use serde::Deserialize;
use std::collections::VecDeque;
fn glyph_at(frame: &Frame, row: usize, col: usize) -> Option<&crate::screen::Glyph> {
    frame
        .glyphs
        .binary_search_by_key(&(row, col), |g| (g.row, g.col))
        .ok()
        .and_then(|i| frame.glyphs.get(i))
}
#[cfg(any(test, feature = "desktop"))]
#[doc(hidden)]
pub mod fixture;
#[cfg(test)]
pub(crate) mod tests;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Color {
    pub foreground: u8,
    pub background: u8,
}
impl Color {
    pub fn sgr(self) -> Result<String, String> {
        if self.foreground > 15 || self.background > 7 {
            return Err(crate::msg!("editor-color-invalid"));
        }
        Ok(format!(
            "\x1b[0;{}{};{}m",
            if self.foreground >= 8 { "1;" } else { "" },
            30 + self.foreground % 8,
            40 + self.background
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    pub line: usize,
    pub col: usize,
    pub preview: bool,
    pub insert: bool,
    pub top: usize,
}
pub fn status(frame: &Frame) -> Option<Status> {
    if !navigation::article_editor(frame) {
        return None;
    }
    let rows = text::rows(frame)?;
    let footer = &rows[23].text;
    let (head, position) = footer.rsplit_once('║')?;
    let (line, col) = position.trim().split_once(':')?;
    let line = line.trim().parse::<usize>().ok()?.checked_sub(1)?;
    let col = col.trim().parse::<usize>().ok()?.checked_sub(1)?;
    let (head, modes) = head.rsplit_once('│')?;
    let modes = modes.trim().as_bytes();
    if modes.len() != 4
        || !matches!(modes[0], b'A' | b'a')
        || !matches!(modes[1], b'I' | b'i')
        || !matches!(modes[2], b'P' | b'p')
        || !matches!(modes[3], b'R' | b'r')
        || modes[2] == b'P'
        || modes[3] == b'R'
        || col > 510
        || line > 9_999_998
    {
        return None;
    }
    let insert = if head.ends_with("插入") {
        true
    } else if head.ends_with("取代") {
        false
    } else {
        return None;
    };
    Some(Status {
        line,
        col,
        preview: modes[0] == b'A',
        insert,
        top: line.checked_sub(frame.cursor.1)?,
    })
}

#[derive(Clone, Debug)]
struct Token {
    start: usize,
    end: usize,
    visible: usize,
    width: usize,
    text: String,
    sgr: Option<Vec<u8>>,
}
#[derive(Clone)]
struct Row {
    line: usize,
    span: Span,
    raw: String,
    tokens: Vec<Token>,
    len: usize,
    display: Vec<(String, usize)>,
    expected_styles: Vec<(usize, usize, Style, Style)>,
}
impl Row {
    fn capture(frame: &Frame, state: Status, span: Span, line: usize) -> Result<Self, String> {
        // PTT's code view displays only the first 79 cells of a logical line.
        // End's authoritative column proves we have the complete source row.
        if state.preview || state.col > 79 {
            return Err(crate::msg!("editor-color-long-line"));
        }
        let rows = text::rows(frame).ok_or_else(|| crate::msg!("editor-color-unsupported"))?;
        let row = rows
            .get(
                line.checked_sub(state.top)
                    .ok_or_else(|| crate::msg!("editor-color-stale"))?,
            )
            .ok_or_else(|| crate::msg!("editor-color-stale"))?;
        if !row.mapped() {
            return Err(crate::msg!("editor-color-unsupported"));
        }
        let mut tokens = Vec::new();
        let mut visible = 0;
        let mut index = 0;
        while index < row.spans.len() && row.spans[index].glyph.col < state.col {
            let g = row.spans[index].glyph;
            if g.col + g.width > state.col {
                return Err(crate::msg!("editor-color-unsupported"));
            }
            if g.text == "*"
                && row
                    .spans
                    .get(index + 1)
                    .is_some_and(|s| s.glyph.text == "[")
            {
                let mut next = index + 2;
                let mut parameters = String::new();
                while next < row.spans.len() && row.spans[next].glyph.col < state.col {
                    let c = &row.spans[next].glyph.text;
                    if c == "m" {
                        break;
                    }
                    if c.len() != 1 || !c.bytes().all(|b| b.is_ascii_digit() || b == b';') {
                        break;
                    }
                    parameters.push_str(c);
                    next += 1;
                }
                if next < row.spans.len()
                    && row.spans[next].glyph.col < state.col
                    && row.spans[next].glyph.text == "m"
                {
                    let values = if parameters.is_empty() {
                        vec![0]
                    } else {
                        parameters
                            .split(';')
                            .map(|s| if s.is_empty() { Ok(0) } else { s.parse::<u8>() })
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(|_| crate::msg!("editor-color-unsupported"))?
                    };
                    let mut check = Attributes::default();
                    check.apply(&values)?;
                    tokens.push(Token {
                        start: g.col,
                        end: row.spans[next].glyph.col + 1,
                        visible,
                        width: 0,
                        text: format!("\x1b[{parameters}m"),
                        sgr: Some(values),
                    });
                    index = next + 1;
                    continue;
                }
                // Ambiguous/incomplete control sequences must not become edits.
                return Err(crate::msg!("editor-color-unsupported"));
            }
            tokens.push(Token {
                start: g.col,
                end: g.col + g.width,
                visible,
                width: g.width,
                text: g.text.clone(),
                sgr: None,
            });
            visible += g.width;
            index += 1;
        }
        let raw = tokens.iter().map(|t| t.text.as_str()).collect::<String>();
        Ok(Self {
            line,
            span,
            raw,
            tokens,
            len: state.col,
            display: row
                .spans
                .iter()
                .filter(|s| s.glyph.col < state.col)
                .map(|s| (s.glyph.text.clone(), s.glyph.width))
                .collect(),
            expected_styles: Vec::new(),
        })
    }
    fn verify(&self, frame: &Frame, state: Status) -> Result<Style, String> {
        let rows = text::rows(frame).ok_or_else(|| crate::msg!("editor-color-stale"))?;
        let row = rows
            .get(
                self.line
                    .checked_sub(state.top)
                    .ok_or_else(|| crate::msg!("editor-color-stale"))?,
            )
            .ok_or_else(|| crate::msg!("editor-color-stale"))?;
        let plain = self
            .tokens
            .iter()
            .filter(|t| t.sgr.is_none())
            .map(|t| t.text.as_str())
            .collect::<String>();
        if !row.mapped() || row.text.trim_end_matches(' ') != plain.trim_end_matches(' ') {
            return Err(crate::msg!("editor-color-unsupported"));
        }
        Ok(row.spans[0].glyph.left)
    }
    fn edits(
        &self,
        color: Color,
        original_preview: bool,
        base: Style,
    ) -> Result<Vec<Edit>, String> {
        let chosen = self
            .tokens
            .iter()
            .filter(|t| {
                t.sgr.is_none() && {
                    let start = if original_preview { t.visible } else { t.start };
                    let end = start + t.width;
                    start < self.span.end && end > self.span.start
                }
            })
            .collect::<Vec<_>>();
        if chosen
            .iter()
            .all(|t| t.text.chars().all(char::is_whitespace))
        {
            return Ok(Vec::new());
        }
        let first = chosen
            .first()
            .ok_or_else(|| crate::msg!("editor-color-empty"))?;
        let last = chosen
            .last()
            .ok_or_else(|| crate::msg!("editor-color-empty"))?;
        let tail = self
            .tokens
            .iter()
            .any(|t| t.sgr.is_none() && t.start >= last.end);
        let end = if tail { last.end } else { self.len };
        let mut attributes = Attributes {
            style: base,
            ..Default::default()
        };
        for t in self.tokens.iter().filter(|t| t.end <= last.end) {
            if let Some(values) = &t.sgr {
                attributes.apply(values)?;
            }
        }
        let close = if tail {
            format!("\x1b[m{}", attributes.sgr())
        } else {
            "\x1b[m".to_owned()
        };
        let code = color.sgr()?;
        let mut edits = vec![Edit {
            line: self.line,
            col: end,
            code: close,
        }];
        for token in self
            .tokens
            .iter()
            .rev()
            .filter(|t| t.sgr.is_some() && t.start >= first.start && t.end <= last.start)
        {
            edits.push(Edit {
                line: self.line,
                col: token.end,
                code: code.clone(),
            });
        }
        edits.push(Edit {
            line: self.line,
            col: first.start,
            code,
        });
        // WRAPMARGIN is 511 in PTT. Stay strictly below it, including old SGR.
        if self.len + edits.iter().map(|e| e.code.len()).sum::<usize>() >= 511 {
            return Err(crate::msg!("editor-color-long-line"));
        }
        Ok(edits)
    }
    fn check_raw(&self, frame: &Frame, state: Status) -> Result<(), String> {
        let row = self
            .line
            .checked_sub(state.top)
            .ok_or_else(|| crate::msg!("editor-color-stale"))?;
        if row >= 23 {
            return Err(crate::msg!("editor-color-stale"));
        }
        let mut col = 0;
        for (value, width) in &self.display {
            // Ignore only the editor's rightmost clipping boundary; all text
            // before that must still match the retained, locally updated row.
            if col + width > 78 {
                break;
            }
            if !glyph_at(frame, row, col)
                .is_some_and(|g| g.width == *width && g.text == *value && !g.unmapped)
            {
                return Err(crate::msg!("editor-color-stale"));
            }
            col += width;
        }
        Ok(())
    }
    fn insert(&mut self, col: usize, code: &str) -> Result<(), String> {
        let mut position = 0;
        let index = self
            .display
            .iter()
            .position(|(_, width)| {
                let here = position;
                position += width;
                here == col
            })
            .or_else(|| (position == col).then_some(self.display.len()))
            .ok_or_else(|| crate::msg!("editor-color-stale"))?;
        self.display.splice(
            index..index,
            code.chars()
                .map(|c| ((if c == '\x1b' { '*' } else { c }).to_string(), 1)),
        );
        Ok(())
    }
    fn expect_styles(
        &mut self,
        frame: &Frame,
        state: Status,
        color: Color,
        preview_selection: bool,
    ) -> Result<(), String> {
        let selected = |t: &Token| {
            let start = if preview_selection {
                t.visible
            } else {
                t.start
            };
            start < self.span.end && start + t.width > self.span.start
        };
        let actual_row = self
            .line
            .checked_sub(state.top)
            .ok_or_else(|| crate::msg!("editor-color-stale"))?;
        let changes = self
            .tokens
            .iter()
            .filter(|t| t.sgr.is_none() && selected(t))
            .any(|t| !t.text.chars().all(char::is_whitespace));
        for t in self.tokens.iter().filter(|t| t.sgr.is_none()) {
            let glyph = glyph_at(frame, actual_row, t.visible)
                .filter(|g| g.width == t.width)
                .ok_or_else(|| crate::msg!("editor-color-stale"))?;
            let (left, right) = if changes && selected(t) {
                let style = Style {
                    fg: color.foreground % 8,
                    bg: color.background,
                    bright: color.foreground >= 8,
                    inverse: false,
                };
                (style, style)
            } else {
                (glyph.left, glyph.right)
            };
            self.expected_styles.push((t.visible, t.width, left, right));
        }
        Ok(())
    }
    fn verify_styles(&self, frame: &Frame, state: Status) -> Result<(), String> {
        let row = self
            .line
            .checked_sub(state.top)
            .ok_or_else(|| crate::msg!("editor-color-stale"))?;
        for &(col, width, left, right) in &self.expected_styles {
            if !glyph_at(frame, row, col)
                .is_some_and(|g| g.width == width && g.left == left && g.right == right)
            {
                return Err(crate::msg!("editor-color-stale"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
struct Attributes {
    style: Style,
    underline: bool,
    blink: bool,
}
impl Attributes {
    fn apply(&mut self, values: &[u8]) -> Result<(), String> {
        for &v in values {
            match v {
                0 => *self = Self::default(),
                1 => self.style.bright = true,
                22 => self.style.bright = false,
                4 => self.underline = true,
                24 => self.underline = false,
                5 => self.blink = true,
                25 => self.blink = false,
                7 => self.style.inverse = true,
                27 => self.style.inverse = false,
                30..=37 => self.style.fg = v - 30,
                39 => self.style.fg = 7,
                40..=47 => self.style.bg = v - 40,
                49 => self.style.bg = 0,
                90..=97 => self.style.fg = v - 90 + 8,
                100..=107 => self.style.bg = v - 100 + 8,
                _ => return Err(crate::msg!("editor-color-unsupported")),
            }
        }
        Ok(())
    }
    fn sgr(&self) -> String {
        format!(
            "\x1b[0;{};{};{};{};{};{}m",
            if self.style.bright { 1 } else { 22 },
            if self.underline { 4 } else { 24 },
            if self.blink { 5 } else { 25 },
            if self.style.inverse { 7 } else { 27 },
            if self.style.fg < 8 {
                30 + self.style.fg
            } else {
                90 + self.style.fg - 8
            },
            if self.style.bg < 8 {
                40 + self.style.bg
            } else {
                100 + self.style.bg - 8
            }
        )
    }
}
#[derive(Clone)]
struct Edit {
    line: usize,
    col: usize,
    code: String,
}
enum Stage {
    ClearBlock,
    Raw,
    Inspect(usize),
    Preview,
    Plan,
    Editing,
    Return,
    Finish,
}
struct Await {
    revision: u64,
    line: usize,
    col: Option<usize>,
    preview: bool,
    insert: bool,
}
pub enum Action {
    Send(Vec<u8>),
    Wait,
    Done(Result<(), String>),
}
pub struct Flow {
    original: Status,
    spans: Vec<Span>,
    rows: Vec<Row>,
    color: Color,
    stage: Stage,
    edits: VecDeque<Edit>,
    awaiting: Option<Await>,
    failure: Option<String>,
    changed: bool,
    original_rows: Vec<String>,
    return_col: usize,
    move_goal: Option<(usize, usize)>,
}
impl Flow {
    pub fn new(frame: &Frame, selection: &Selection, color: Color) -> Result<Self, String> {
        color.sgr()?;
        let original = status(frame).ok_or_else(|| crate::msg!("editor-color-unsupported"))?;
        let selected = selection::extract(frame, selection)?;
        if selected.rows.end > 23 || selected.spans.is_empty() {
            return Err(crate::msg!("editor-color-empty"));
        }
        let rows = text::rows(frame).ok_or_else(|| crate::msg!("editor-color-unsupported"))?;
        let original_rows = selected
            .spans
            .iter()
            .map(|s| rows[s.row].text.trim_end_matches(' ').to_owned())
            .collect();
        Ok(Self {
            original,
            spans: selected.spans,
            rows: Vec::new(),
            color,
            stage: Stage::ClearBlock,
            edits: VecDeque::new(),
            awaiting: None,
            failure: None,
            changed: false,
            original_rows,
            return_col: original.col,
            move_goal: None,
        })
    }
    pub fn changed(&self) -> bool {
        self.changed
    }
    fn send(
        &mut self,
        frame: &Frame,
        bytes: Vec<u8>,
        line: usize,
        col: Option<usize>,
        preview: bool,
        insert: bool,
    ) -> Action {
        self.awaiting = Some(Await {
            revision: frame.revision,
            line,
            col,
            preview,
            insert,
        });
        Action::Send(bytes)
    }
    fn mode(
        &mut self,
        frame: &Frame,
        state: Status,
        preview: bool,
        insert: bool,
    ) -> Option<Action> {
        let mut bytes = Vec::new();
        if state.preview != preview {
            bytes.push(0x16);
        }
        if state.insert != insert {
            bytes.push(0x0f);
        }
        (!bytes.is_empty()).then(|| self.send(frame, bytes, state.line, None, preview, insert))
    }
    fn line(&mut self, frame: &Frame, state: Status, line: usize) -> Option<Action> {
        if state.line == line {
            return None;
        }
        let key = if state.line < line {
            b"\x1b[B"
        } else {
            b"\x1b[A"
        };
        let bytes = key.repeat(state.line.abs_diff(line));
        Some(self.send(frame, bytes, line, None, state.preview, state.insert))
    }
    fn column(&mut self, frame: &Frame, state: Status, col: usize) -> Option<Action> {
        if state.col == col {
            return None;
        }
        let mut count = 1;
        if !state.preview
            && let Some(row) = self
                .rows
                .iter()
                .find(|r| r.line == state.line && r.len != usize::MAX)
        {
            let end = row.display.iter().map(|(_, w)| *w).sum::<usize>();
            if col == end || end.saturating_sub(col) < state.col.abs_diff(col) / 2 {
                return Some(self.send(
                    frame,
                    vec![5],
                    state.line,
                    Some(end),
                    state.preview,
                    state.insert,
                ));
            }
            if col < state.col.abs_diff(col) / 2 {
                return Some(self.send(
                    frame,
                    vec![1],
                    state.line,
                    Some(0),
                    state.preview,
                    state.insert,
                ));
            }
            let mut offset = 0;
            let mut boundaries = Vec::new();
            for (_, width) in &row.display {
                boundaries.push((offset, *width));
                offset += width;
            }
            let (low, high) = (col.min(state.col), col.max(state.col));
            if boundaries
                .iter()
                .filter(|(c, w)| *c < high && c + w > low)
                .all(|(_, w)| *w == 1)
            {
                count = high - low;
            }
        }
        let (bytes, expected) = if col == 0 {
            (vec![1], Some(0))
        } else {
            // Each cursor reply proves the station's DBCS stepping. Never assume
            // that one Right equals one byte or one Unicode character.
            (
                if state.col < col {
                    b"\x1b[C".repeat(count)
                } else {
                    b"\x1b[D".repeat(count)
                },
                (count > 1).then_some(col),
            )
        };
        if expected.is_none() {
            self.move_goal = Some((state.col, col));
        }
        Some(self.send(
            frame,
            bytes,
            state.line,
            expected,
            state.preview,
            state.insert,
        ))
    }
    pub fn next(&mut self, frame: &Frame) -> Result<Action, String> {
        let Some(state) = status(frame) else {
            // A delayed fragment may temporarily remove the footer. It grants
            // no input and does not extend the connection owner's deadline.
            return if self.awaiting.is_some() {
                Ok(Action::Wait)
            } else {
                Err(crate::msg!("editor-color-stale"))
            };
        };
        if let Some(wait) = &self.awaiting {
            if frame.revision == wait.revision
                || state.top != self.original.top
                || state.line != wait.line
                || state.preview != wait.preview
                || state.insert != wait.insert
                || wait.col.is_some_and(|c| c != state.col)
            {
                return Ok(Action::Wait);
            }
            if let Some((from, to)) = self.move_goal {
                if state.col == from {
                    return Ok(Action::Wait);
                }
                if from < to && (state.col < from || state.col > to)
                    || from > to && (state.col > from || state.col < to)
                {
                    return Err(crate::msg!("editor-color-stale"));
                }
            }
            self.move_goal = None;
            self.awaiting = None;
        }
        // All navigation is confined to the initial 23 visible logical lines.
        // An unrelated cursor/footer can never allocate a large repeat buffer.
        if state.top != self.original.top {
            return Err(crate::msg!("editor-color-stale"));
        }
        match self.step(frame, state) {
            Ok(action) => Ok(action),
            Err(error) if !self.changed => {
                self.failure = Some(error);
                self.stage = Stage::Return;
                self.step(frame, state)
            }
            Err(error) => Err(error),
        }
    }
    fn step(&mut self, frame: &Frame, state: Status) -> Result<Action, String> {
        match self.stage {
            Stage::ClearBlock => {
                // Ctrl+V with PTT's own block mark can convert literal *[ into
                // ESC. Revoke that station mark before switching preview modes;
                // the application's acknowledged mouse selection is separate.
                self.stage = Stage::Raw;
                Ok(self.send(
                    frame,
                    b"\x1bu".to_vec(),
                    state.line,
                    Some(state.col),
                    state.preview,
                    state.insert,
                ))
            }
            Stage::Raw => {
                if let Some(a) = self.mode(frame, state, false, state.insert) {
                    return Ok(a);
                }
                self.stage = Stage::Inspect(0);
                self.step(frame, state)
            }
            Stage::Inspect(index) => {
                if index == self.spans.len() {
                    self.stage = Stage::Preview;
                    return self.step(frame, state);
                }
                let span = self.spans[index].clone();
                let line = self.original.top + span.row;
                if let Some(a) = self.line(frame, state, line) {
                    return Ok(a);
                }
                // One separate End response gives the source length, including
                // trailing spaces and hidden SGRs. Capture on the following step.
                if self.rows.len() == index {
                    self.rows.push(Row {
                        line,
                        span,
                        raw: String::new(),
                        tokens: Vec::new(),
                        len: usize::MAX,
                        display: Vec::new(),
                        expected_styles: Vec::new(),
                    });
                    return Ok(self.send(frame, vec![5], line, None, false, state.insert));
                }
                self.rows[index] = Row::capture(frame, state, span, line)?;
                let observed = if self.original.preview {
                    self.rows[index]
                        .tokens
                        .iter()
                        .filter(|t| t.sgr.is_none())
                        .map(|t| t.text.as_str())
                        .collect::<String>()
                } else {
                    self.rows[index].raw.replace('\x1b', "*")
                };
                if observed.trim_end_matches(' ') != self.original_rows[index] {
                    return Err(crate::msg!("editor-color-stale"));
                }
                self.stage = Stage::Inspect(index + 1);
                self.step(frame, state)
            }
            Stage::Preview => {
                if let Some(a) = self.mode(frame, state, true, state.insert) {
                    return Ok(a);
                }
                self.stage = Stage::Plan;
                self.step(frame, state)
            }
            Stage::Plan => {
                for row in self.rows.iter_mut().rev() {
                    let base = row.verify(frame, state)?;
                    row.expect_styles(frame, state, self.color, self.original.preview)?;
                    self.edits
                        .extend(row.edits(self.color, self.original.preview, base)?);
                }
                if !self.original.preview {
                    self.return_col += self
                        .edits
                        .iter()
                        .filter(|e| e.line == self.original.line && e.col <= self.original.col)
                        .map(|e| e.code.len())
                        .sum::<usize>();
                }
                if self.edits.is_empty() {
                    return Err(crate::msg!("editor-color-empty"));
                }
                self.stage = Stage::Editing;
                self.step(frame, state)
            }
            Stage::Editing => {
                if let Some(a) = self.mode(frame, state, false, true) {
                    return Ok(a);
                }
                for row in &self.rows {
                    row.check_raw(frame, state)?;
                }
                let Some(edit) = self.edits.front().cloned() else {
                    self.stage = Stage::Return;
                    return self.step(frame, state);
                };
                if let Some(a) = self.line(frame, state, edit.line) {
                    return Ok(a);
                }
                if let Some(a) = self.column(frame, state, edit.col) {
                    return Ok(a);
                }
                self.edits.pop_front();
                let row = self
                    .rows
                    .iter_mut()
                    .find(|r| r.line == edit.line)
                    .ok_or_else(|| crate::msg!("editor-color-stale"))?;
                row.insert(edit.col, &edit.code)?;
                self.changed = true;
                let bytes = edit
                    .code
                    .bytes()
                    .map(|b| if b == 0x1b { 0x15 } else { b })
                    .collect();
                Ok(self.send(
                    frame,
                    bytes,
                    edit.line,
                    Some(edit.col + edit.code.len()),
                    false,
                    true,
                ))
            }
            Stage::Return => {
                if let Some(a) = self.mode(frame, state, true, self.original.insert) {
                    return Ok(a);
                }
                // Verify text after all writes, before reporting completion.
                if self.failure.is_none() {
                    for row in &self.rows {
                        row.verify(frame, state)?;
                        row.verify_styles(frame, state)?;
                    }
                }
                if let Some(a) = self.line(frame, state, self.original.line) {
                    return Ok(a);
                }
                self.stage = Stage::Finish;
                if !self.original.preview {
                    return Ok(self.send(frame, vec![0x16], state.line, None, false, state.insert));
                }
                self.step(frame, state)
            }
            Stage::Finish => {
                if let Some(a) = self.column(frame, state, self.return_col) {
                    return Ok(a);
                }
                Ok(Action::Done(self.failure.take().map_or(Ok(()), Err)))
            }
        }
    }
}
