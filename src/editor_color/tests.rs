use super::fixture::Editor;
use super::*;
use crate::{encoding, selection::Position};

fn selection(a: (usize, usize), b: (usize, usize)) -> Selection {
    Selection::Range {
        anchor: Position { row: a.0, col: a.1 },
        focus: Position { row: b.0, col: b.1 },
    }
}
const RED: Color = Color {
    foreground: 9,
    background: 0,
};
fn run(editor: &mut Editor, selection: Selection) -> Result<(), String> {
    let mut flow = Flow::new(&editor.frame(), &selection, RED)?;
    for _ in 0..4096 {
        match flow.next(&editor.frame())? {
            Action::Send(bytes) => editor.accept(&bytes),
            Action::Wait => panic!("unexpected wait after complete synthetic reply"),
            Action::Done(result) => return result,
        }
    }
    panic!("colouring did not finish")
}
#[test]
fn palette_uses_ptt_brightness_and_eight_backgrounds() {
    assert_eq!(RED.sgr().unwrap(), "\x1b[0;1;31;40m");
    assert_eq!(
        Color {
            foreground: 4,
            background: 7
        }
        .sgr()
        .unwrap(),
        "\x1b[0;34;47m"
    );
    assert!(
        Color {
            foreground: 16,
            background: 0
        }
        .sgr()
        .is_err()
    );
    assert!(
        Color {
            foreground: 0,
            background: 8
        }
        .sgr()
        .is_err()
    );
}
#[test]
fn full_visible_chinese_page_uses_bounded_commands_and_keeps_every_line() {
    let line = format!("{}A", "中".repeat(39));
    let values = vec![line.as_str(); 23];
    let mut editor = Editor::new(&values, true, false, true);
    run(&mut editor, selection((0, 0), (22, 78))).unwrap();
    let expected = encoding::encode(&format!("\x1b[0;1;31;40m{line}\x1b[m")).unwrap();
    assert!(editor.lines.iter().all(|l| l == &expected));
    assert!(
        editor.commands.len() < 200,
        "one network round trip per character would make page colouring unresponsive"
    );
}
#[test]
fn multiline_wraps_each_nonempty_row_and_leaves_blank_rows_untouched() {
    for preview in [true, false] {
        for insert in [true, false] {
            for dbcs in [true, false] {
                let mut editor =
                    Editor::new(&["第一行", "", "  ", "第三行"], preview, insert, dbcs);
                run(&mut editor, selection((0, 0), (3, 5))).unwrap();
                let expected = [
                    "\x1b[0;1;31;40m第一行\x1b[m",
                    "",
                    "  ",
                    "\x1b[0;1;31;40m第三行\x1b[m",
                ];
                assert_eq!(
                    editor.lines,
                    expected
                        .iter()
                        .map(|s| encoding::encode(s).unwrap())
                        .collect::<Vec<_>>()
                );
                assert_eq!((editor.preview, editor.insert), (preview, insert));
                assert!(
                    editor
                        .commands
                        .iter()
                        .all(|b| !b.contains(&b'\r') && !b.contains(&0x19) && !b.contains(&0x04))
                );
            }
        }
    }
}
#[test]
fn selecting_half_a_chinese_glyph_colours_the_whole_glyph_only() {
    for dbcs in [true, false] {
        let mut editor = Editor::new(&["Ab中文C"], true, true, dbcs);
        run(&mut editor, selection((0, 3), (0, 3))).unwrap();
        assert_eq!(
            editor.lines[0],
            encoding::encode("Ab\x1b[0;1;31;40m中\x1b[m\x1b[0;22;24;25;27;37;40m文C").unwrap()
        );
    }
}
#[test]
fn old_sgr_inside_selection_is_overridden_and_tail_keeps_its_colour() {
    for preview in [true, false] {
        let mut editor = Editor::new(&["P\x1b[32m中\x1b[0m文\x1b[34mQ"], preview, false, true);
        let range = if preview {
            selection((0, 1), (0, 4))
        } else {
            selection((0, 6), (0, 13))
        };
        run(&mut editor, range).unwrap();
        assert_eq!(editor.lines[0],encoding::encode("P\x1b[32m\x1b[0;1;31;40m中\x1b[0m\x1b[0;1;31;40m文\x1b[m\x1b[0;22;24;25;27;37;40m\x1b[34mQ").unwrap());
        editor.preview = true;
        let frame = editor.frame();
        let glyphs = frame
            .glyphs
            .iter()
            .filter(|g| g.row == 0)
            .collect::<Vec<_>>();
        assert_eq!(
            (
                glyphs[0].left.fg,
                glyphs[1].left.fg,
                glyphs[1].left.bright,
                glyphs[2].left.fg,
                glyphs[3].left.fg
            ),
            (7, 1, true, 1, 4)
        );
    }
}
#[test]
fn long_unknown_and_ambiguous_codes_are_rejected_before_document_mutation() {
    for line in [
        "A".repeat(80),
        "\x1b[31;66;32m中文".into(),
        "literal *[31m text".into(),
    ] {
        let mut editor = Editor::new(&[&line], true, false, true);
        let before = editor.lines.clone();
        assert!(run(&mut editor, selection((0, 0), (0, 5))).is_err());
        assert_eq!(editor.lines, before);
        assert!(!editor.insert);
        assert!(editor.preview);
    }
}
#[test]
fn footer_or_screen_change_never_creates_an_edit() {
    let mut editor = Editor::new(&["unchanged"], true, true, true);
    let frame = editor.frame();
    assert!(Flow::new(&frame, &selection((23, 0), (23, 4)), RED).is_err());
    let mut flow = Flow::new(&frame, &selection((0, 0), (0, 5)), RED).unwrap();
    let Action::Send(bytes) = flow.next(&frame).unwrap() else {
        panic!()
    };
    editor.accept(&bytes);
    editor.lines[0] = b"changed".to_vec();
    for _ in 0..100 {
        match flow.next(&editor.frame()).unwrap() {
            Action::Send(bytes) => editor.accept(&bytes),
            Action::Done(result) => {
                assert!(result.is_err());
                assert!(!flow.changed());
                return;
            }
            Action::Wait => panic!(),
        }
    }
    panic!("did not stop");
}
#[test]
fn incomplete_reply_waits_without_editing_and_changed_text_stops_after_a_write() {
    let mut editor = Editor::new(&["abc"], true, true, true);
    let frame = editor.frame();
    let mut flow = Flow::new(&frame, &Selection::Line { row: 0 }, RED).unwrap();
    let Action::Send(bytes) = flow.next(&frame).unwrap() else {
        panic!()
    };
    editor.accept(&bytes);
    let mut partial = editor.frame();
    partial.control_complete = false;
    assert!(matches!(flow.next(&partial).unwrap(), Action::Wait));
    assert!(!flow.changed());
    for _ in 0..100 {
        match flow.next(&editor.frame()).unwrap() {
            Action::Send(bytes) => {
                editor.accept(&bytes);
                if flow.changed() {
                    editor.lines[0][0] = b'Z';
                    assert!(flow.next(&editor.frame()).is_err());
                    return;
                }
            }
            _ => panic!("expected a bounded command"),
        }
    }
    panic!("missing mutation");
}
