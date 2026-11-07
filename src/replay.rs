//! Self-authored display fixture shared by the offline and native loopback probes.
use crate::{
    encoding,
    screen::{Frame, Style},
};

pub fn editor_clipboard() -> Result<Vec<u8>, encoding::Unencodable> {
    encoding::encode(
        "\x1b[2J\x1b[HEDITOR CLIPBOARD 0/4 - local only\x1b[5;1H\x1b[1;31mANSI colour selection\x1b[0m\x1b[24;1H\x1b[34;46m 編輯文章 \x1b[30;47m (^X)存檔 (^C)色碼 (Esc-h)按鍵 (^Z)說明 插入 | user | 1:1\x1b[0m\x1b[3;1H",
    )
}

pub fn parity_grid() -> Result<Vec<u8>, encoding::Unencodable> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"\x1b[1;38H\x1b[1;36mPTTStella\x1b[0m\x1b[3;5H");
    bytes.extend_from_slice(&encoding::encode("PTTStella 終端重放  /  固定 80 x 24")?);
    bytes.extend_from_slice(
        b"\x1b[5;5HASCII [] gjpqy_ () { }\x1b[7;5H\x1b[91;44;66;92;45m\xa4\xa4\x1b[0m ",
    );
    bytes.extend_from_slice(&encoding::encode("雙色中文字  /  左右半格分開上色")?);
    bytes.extend_from_slice(b"\x1b[9;5H\x1b[1;36m");
    bytes.extend_from_slice(&encoding::encode("███▌ ◢◣◤◥  ●○◎⊙")?);
    bytes.extend_from_slice(b"\x1b[0m\x1b[11;5H");
    bytes.extend_from_slice(&encoding::encode("┌──┐\x1b[12;5H│    │\x1b[13;5H└──┘")?);
    bytes.extend_from_slice(b"\x1b[15;5HANSI 16: ");
    for color in (40..=47).chain(100..=107) {
        bytes.extend_from_slice(format!("\x1b[{color}m   ").as_bytes());
    }
    bytes.extend_from_slice(b"\x1b[0m\x1b[17;5H\x1b[7m reverse video \x1b[0m\x1b[20;5H");
    bytes.extend_from_slice(&encoding::encode("離線合成資料。視窗縮放保留欄寬與行高。")?);
    bytes.extend_from_slice(b"\x1b[24;1H\x1b[44;97m");
    bytes.extend_from_slice(&encoding::encode("第 24 行")?);
    bytes.extend_from_slice(b"\x1b[K\x1b[0m\x1b[?25l");
    Ok(bytes)
}

pub fn verify_parity_grid(frame: &Frame) -> Result<(), &'static str> {
    if (frame.cols, frame.rows) != (80, 24) || frame.cursor_visible || !frame.control_complete {
        return Err("parity grid dimensions, cursor or control state differs");
    }
    let glyph = |col, row| {
        frame
            .glyphs
            .iter()
            .find(|glyph| glyph.col == col && glyph.row == row)
    };
    let chinese = glyph(4, 6).ok_or("parity grid is missing the split glyph")?;
    if chinese.text != "中"
        || chinese.width != 2
        || chinese.left
            != (Style {
                fg: 9,
                bg: 4,
                bright: false,
                inverse: false,
            })
        || chinese.right
            != (Style {
                fg: 10,
                bg: 5,
                bright: false,
                inverse: false,
            })
    {
        return Err("parity grid split glyph differs");
    }
    let last = glyph(79, 23).ok_or("parity grid is missing the last cell")?;
    if last.text != " " || last.left.bg != 4 {
        return Err("parity grid last cell differs");
    }
    if glyph(4, 8).is_none_or(|g| g.text != "█") || glyph(4, 4).is_none_or(|g| g.text != "A") {
        return Err("parity grid artwork or Latin text differs");
    }
    Ok(())
}

/// Use the shipped fonts in both native probes, independent of installed fonts.
pub fn parity_display_profile() -> crate::settings::DisplayProfile {
    crate::settings::DisplayProfile {
        fonts: crate::settings::Fonts::Preset {
            preset: crate::settings::Preset::Bundled,
        },
        ..crate::settings::DisplayProfile::default()
    }
}

// Shared self-authored synthetic reading workload for both native hosts.
pub fn full_load_page(page: usize, origin: Option<&str>) -> Result<Vec<u8>, String> {
    reading_load_page(page, 20, false, origin)
}
pub fn long_load_page(page: usize, origin: Option<&str>) -> Result<Vec<u8>, String> {
    reading_load_page(page, 38, true, origin)
}
pub fn native_article_input_page(page: usize) -> Result<Vec<u8>, String> {
    reading_load_page(page, 2, false, Some("http://127.0.0.1:9"))
}
/// Complete self-authored page: interactive probes must not need preload keys.
pub fn native_article_interaction_page() -> Result<Vec<u8>, String> {
    reading_load_page(1, 1, false, Some("http://127.0.0.1:9"))
}
/// Self-authored output fixture: history-only dual-colour text and a redacted
/// push URL; the footer is an OSC 8 link independent of its visible label.
pub fn native_article_output_page(page: usize) -> Result<Vec<u8>, String> {
    let mut bytes = native_article_input_page(page)?;
    if page == 1 {
        let patch = "\x1b[1;1H\x1b[2K\x1b[0m作者  Alice                                      看板  Test\x1b[5;1H\x1b[2K\x1b[1;37m推 \x1b[1;33mBlocked\x1b[0;33m:\x1b[0;37m hidden https://example.com/private\x1b[5;67H 01/01 12:34\x1b[6;1H\x1b[2K\x1b[0mSplit \x1b[31;44m";
        bytes.extend(encoding::encode(patch).map_err(|e| e.to_string())?);
        bytes.extend(b"\xa4\x1b[32;45m\xa4\x1b[0m");
    } else {
        bytes.extend(encoding::encode("\x1b[5;1H\x1b[2K\x1b[1;37m推 \x1b[1;33mBlocked\x1b[0;33m:\x1b[0;37m hidden https://example.com/private\x1b[5;67H 01/01 12:34\x1b[0m").map_err(|e| e.to_string())?);
    }
    bytes.extend(
        encoding::encode(
            "\x1b[24;2H\x1b[30;47m\x1b]8;;https://example.com/footer\x1b\\瀏覽\x1b]8;;\x1b\\\x1b[0m\x1b[24;80H",
        )
        .map_err(|e| e.to_string())?,
    );
    Ok(bytes)
}
fn reading_load_page(
    page: usize,
    total: usize,
    reset_ink: bool,
    origin: Option<&str>,
) -> Result<Vec<u8>, String> {
    if page == 0 || page > total {
        return Err("reading fixture page out of range".into());
    }
    let first = 1 + (page - 1) * 22;
    let mut text = String::from("\x1b[2J\x1b[H");
    for row in 0..23 {
        let line = first + row;
        let body = if line == 2 {
            format!(
                "{}/colours.png",
                origin.ok_or("full load probe requires media origin")?
            )
        } else if line == 5 {
            format!(
                "{}/missing.png",
                origin.ok_or("full load probe requires media origin")?
            )
        } else if line == 270 {
            format!(
                "{}/colours-two.png",
                origin.ok_or("full load probe requires media origin")?
            )
        } else {
            format!("Full article synthetic line {line:04}")
        };
        let ink = if reset_ink && page == 30 && row == 22 {
            "\x1b[1;33m"
        } else {
            "\x1b[0m"
        };
        text.push_str(&format!("\x1b[{};1H{ink}{body}", row + 1));
    }
    text.push_str(&format!("\x1b[24;1H\x1b[30;47m 瀏覽 第 {page}/{total} 頁 ({}%) 目前顯示: 第 {first}~{} 行 (y)回應(X)推文(h)說明\x1b[0m\x1b[24;80H",page*100/total,first+22));
    encoding::encode(&text).map_err(|e| e.to_string())
}

#[cfg(all(test, feature = "desktop"))]
mod reading_interaction_tests {
    use super::*;
    use crate::{
        article_cache::{ArticleCache, LoadState},
        presentation,
        screen::Screen,
    };
    #[test]
    fn complete_interaction_page_has_no_missing_edge() {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&native_article_interaction_page().unwrap());
        let frame = screen.frame();
        assert_eq!(
            crate::navigation::analyze(&frame),
            crate::navigation::Context::ArticleReading {
                first_line: 1,
                last_line: 23,
                percent: 100
            }
        );
        for (row, text) in [
            (0, "Full article synthetic line 0001"),
            (22, "Full article synthetic line 0023"),
        ] {
            assert_eq!(
                crate::selection::extract(&frame, &crate::selection::Selection::Line { row })
                    .unwrap()
                    .text
                    .trim_end(),
                text
            );
        }
    }
    #[test]
    fn output_fixture_keeps_reading_commands_push_styles_and_both_colour_halves() {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&native_article_output_page(1).unwrap());
        let frame = screen.frame();
        assert!(matches!(
            crate::navigation::analyze(&frame),
            crate::navigation::Context::ArticleReading {
                first_line: 1,
                last_line: 23,
                percent: 50
            }
        ));
        let pushes = crate::filters::pushes(&frame);
        assert_eq!(pushes.len(), 1);
        assert_eq!((pushes[0].row, pushes[0].id.as_str()), (4, "Blocked"));
        let split = frame
            .glyphs
            .iter()
            .find(|g| g.row == 5 && g.col == 6)
            .unwrap();
        assert_eq!(split.text, "中");
        assert_eq!(
            (split.left.fg, split.left.bg, split.right.fg, split.right.bg),
            (1, 4, 2, 5)
        );
        let links = crate::links::detect(&frame);
        assert!(
            links
                .iter()
                .any(|l| l.row == 23 && l.start == 1 && l.url == "https://example.com/footer")
        );
        screen.feed(&native_article_output_page(2).unwrap());
        let pushes = crate::filters::pushes(&screen.frame());
        assert_eq!(pushes.len(), 1);
        assert_eq!((pushes[0].row, pushes[0].id.as_str()), (4, "Blocked"));
        assert!(matches!(
            crate::navigation::analyze(&screen.frame()),
            crate::navigation::Context::ArticleReading {
                first_line: 23,
                last_line: 45,
                percent: 100
            }
        ));
    }

    #[test]
    fn output_fixture_joins_two_pages_without_changing_the_overlap_background() {
        let mut cache = ArticleCache::default();
        let mut screen = Screen::new(80, 24).unwrap();
        let display = parity_display_profile().resolve().unwrap();
        for n in 1..=2 {
            screen.feed(&native_article_output_page(n).unwrap());
            let frame = screen.frame();
            let mut paint = presentation::sample_packet(&frame, &display);
            paint.connected = true;
            paint.ptt_features = true;
            paint.generation = 1;
            cache.capture_source(&paint, Some(&frame));
        }
        assert_eq!(
            cache
                .progress()
                .map(|(_, first, last, state)| (first, last, state)),
            Some((1, 45, Some(LoadState::Complete)))
        );
    }
}
