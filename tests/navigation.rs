use pttstella::{
    encoding,
    navigation::{self, Context, Error, Intent, UnknownReason},
    screen::{Frame, Screen},
    text,
};

fn fixture(name: &str) -> Screen {
    let pages: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/navigation.json")).unwrap();
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&encoding::encode(pages[name].as_str().unwrap()).unwrap());
    screen
}
fn page(name: &str) -> Context {
    navigation::analyze(&fixture(name).frame())
}
fn unknown(frame: &Frame) {
    assert!(matches!(
        navigation::analyze(frame),
        Context::Unknown { .. }
    ));
}
fn replace(screen: &mut Screen, row: usize, text: &str, sgr: &str) {
    screen.feed(
        &encoding::encode(&format!(
            "\x1b[{};1H\x1b[{sgr}m\x1b[2K{text}\x1b[0m",
            row + 1
        ))
        .unwrap(),
    );
}

fn article_row(number: &str, flags: &str, date: &str, author: &str, title: &str) -> String {
    let number = if number == "★" {
        "   ★ ".to_owned()
    } else {
        format!("{number:>6}")
    };
    format!(" {number}{flags}{date} {author:13}{title}")
}

#[test]
fn list_anchors_and_cursor_produce_only_number_targets() {
    let Context::ArticleList {
        articles,
        cursor_row,
    } = page("list")
    else {
        panic!("expected list");
    };
    assert_eq!(cursor_row, 4);
    assert_eq!(
        articles
            .iter()
            .map(|a| (a.row, a.start, a.end))
            .collect::<Vec<_>>(),
        [(3, 2, 8), (4, 2, 8), (5, 2, 8)]
    );
}
#[test]
fn opening_same_up_and_down_rows_has_exact_independent_expected_bytes() {
    let context = page("list");
    for (row, expected) in [
        (3, b"\x1b[A\r".as_slice()),
        (4, b"\r".as_slice()),
        (5, b"\x1b[B\r".as_slice()),
    ] {
        assert_eq!(
            navigation::plan(&context, Intent::Open { row }).unwrap(),
            expected
        );
    }
    for row in [0, 2, 6, 23, usize::MAX] {
        assert_eq!(
            navigation::plan(&context, Intent::Open { row }),
            Err(Error::Unavailable)
        );
    }
}
#[test]
fn list_wheel_is_an_item_step_and_reading_actions_are_not_enabled() {
    let context = page("list");
    assert_eq!(
        navigation::plan(&context, Intent::PreviousItem).unwrap(),
        b"\x1b[A"
    );
    assert_eq!(
        navigation::plan(&context, Intent::NextItem).unwrap(),
        b"\x1b[B"
    );
    assert_eq!(navigation::plan(&context, Intent::Back).unwrap(), b"\x1b[D");
    assert_eq!(
        navigation::plan(&context, Intent::NextPage).unwrap(),
        b"\x1b[6~"
    );
    assert_eq!(
        navigation::plan(&context, Intent::PreviousPage).unwrap(),
        b"\x1b[5~"
    );
}
#[test]
fn pinned_article_and_complete_dual_colour_title_keep_column_targets() {
    let mut screen = fixture("list");
    replace(
        &mut screen,
        5,
        &article_row("★", "   8", "10/16", "carol", "□ 合成置底文章"),
        "0",
    );
    screen.feed(b"\x1b[4;40H\x1b[31m\xa4\x1b[32m\xa4\x1b[0m\x1b[5;1H");
    assert!(matches!(
        navigation::analyze(&screen.frame()),
        Context::ArticleList { .. }
    ));
}
#[test]
fn padded_dates_votes_locked_and_deleted_rows_remain_browsable() {
    for text in [
        article_row("122", "   8", " 1/ 9", "carol", "□ 補齊日期"),
        article_row("122", "    ", "10/ 9", "carol", "鎖 鎖定文章"),
        article_row("122", "   8", " 1/19", "carol", "ˇ 合成投票"),
        article_row("122", "    ", " 1/ 9", "-", "╳ (本文已被刪除)"),
        article_row("122", "    ", " 1/ 9", "-", "□ (本文已被刪除)"),
    ] {
        let mut screen = fixture("list");
        replace(&mut screen, 5, &text, "0");
        screen.feed(b"\x1b[5;1H");
        let context = navigation::analyze(&screen.frame());
        assert!(matches!(context, Context::ArticleList { .. }), "{text}");
        assert_eq!(
            navigation::plan(&context, Intent::Open { row: 5 }).unwrap(),
            b"\x1b[B\r"
        );
    }
}
#[test]
fn invalid_padded_dates_and_deleted_author_prompts_remain_unknown() {
    for text in [
        article_row("122", "    ", " 1/ 0", "carol", "□ 日期錯誤"),
        article_row("122", "    ", "13/ 9", "carol", "□ 日期錯誤"),
        article_row("122", "    ", " 1/99", "carol", "□ 日期錯誤"),
        article_row("122", "    ", " 1/ 9", "-", "是否刪除？[Y/n]"),
        article_row("122", "    ", " 1/ 9", "-", "R: 非刪文格式"),
    ] {
        let mut screen = fixture("list");
        replace(&mut screen, 5, &text, "0");
        screen.feed(b"\x1b[5;1H");
        unknown(&screen.frame());
    }
}
#[test]
fn missing_or_inconsistent_anchors_fail_closed() {
    for (row, value, sgr) in [
        (2, "編號 日期 作者 標題", "0"),
        (2, "編號 作 者 標 題", "30;47"),
        (23, "請輸入密碼:", "30;47"),
        (23, "閱讀說明 ← 返回", "30;47"),
    ] {
        let mut screen = fixture("list");
        replace(&mut screen, row, value, sgr);
        screen.feed(b"\x1b[5;1H");
        unknown(&screen.frame());
    }
}
#[test]
fn broken_item_rows_popups_and_interrupted_control_sequences_are_unknown() {
    for value in [
        "您想刪除其他重複登入的連線嗎？[Y/n]".to_owned(),
        article_row("122", "    ", "99/99", "bob", "□ 假日期"),
        article_row("122", "    ", "10/16", "bob", "請輸入密碼"),
        article_row("122", "    ", "10/16", "https://evil", "□ 假作者"),
    ] {
        let mut screen = fixture("list");
        replace(&mut screen, 5, &value, "0");
        screen.feed(b"\x1b[5;1H");
        unknown(&screen.frame());
    }
    let mut screen = fixture("list");
    screen.feed(b"\x1b[");
    assert_eq!(
        navigation::analyze(&screen.frame()),
        Context::Unknown {
            reason: UnknownReason::Incomplete
        }
    );
}
#[test]
fn stale_header_in_article_body_does_not_make_a_list() {
    let mut screen = fixture("reading_middle");
    replace(&mut screen, 2, "編號 日期 作者 標題", "0");
    screen.feed(b"\x1b[24;80H");
    assert!(matches!(
        navigation::analyze(&screen.frame()),
        Context::ArticleReading { .. }
    ));
    replace(
        &mut screen,
        23,
        "文章內容：瀏覽 第 2/3 頁 (66%) 目前顯示: 第 24~46 行 ← 離開",
        "0",
    );
    screen.feed(b"\x1b[24;80H");
    unknown(&screen.frame());
}
#[test]
fn missing_pointer_or_cursor_outside_items_cannot_open_a_list() {
    for cursor in [b"\x1b[1;1H".as_slice(), b"\x1b[5;20H", b"\x1b[7;1H"] {
        let mut screen = fixture("list");
        screen.feed(cursor);
        unknown(&screen.frame());
    }
    let mut screen = fixture("list");
    screen.feed(b"\x1b[5;1H  ");
    screen.feed(b"\x1b[5;1H");
    unknown(&screen.frame());
}
#[test]
fn list_gaps_or_unmapped_bytes_do_not_invent_navigable_rows() {
    let mut screen = fixture("list");
    replace(&mut screen, 4, "", "0");
    screen.feed(b"\x1b[4;1H>");
    screen.feed(b"\x1b[4;1H");
    unknown(&screen.frame());
    let mut screen = fixture("list");
    screen.feed(b"\x1b[6;50H\x81 \x1b[5;1H");
    assert_eq!(
        navigation::analyze(&screen.frame()),
        Context::Unknown {
            reason: UnknownReason::Unmapped
        }
    );
}
#[test]
fn reading_ranges_and_page_boundaries_drive_only_available_actions() {
    assert_eq!(
        page("reading_middle"),
        Context::ArticleReading {
            first_line: 24,
            last_line: 46,
            percent: 66
        }
    );
    assert_eq!(
        navigation::plan(&page("reading_middle"), Intent::PreviousPage).unwrap(),
        b"\x1b[5~"
    );
    assert_eq!(
        navigation::plan(&page("reading_middle"), Intent::NextPage).unwrap(),
        b"\x1b[6~"
    );
    assert_eq!(
        navigation::plan(&page("reading_middle"), Intent::Back).unwrap(),
        b"\x1b[D"
    );
    assert_eq!(
        navigation::plan(&page("reading_first"), Intent::PreviousPage),
        Err(Error::Unavailable)
    );
    assert_eq!(
        navigation::plan(&page("reading_last"), Intent::NextPage),
        Err(Error::Unavailable)
    );
    assert_eq!(
        navigation::plan(&page("reading_middle"), Intent::Open { row: 3 }),
        Err(Error::Unavailable)
    );
}
#[test]
fn malformed_reading_reports_and_prompt_overlays_do_not_enable_actions() {
    for status in [
        "瀏覽 第 0/3 頁 (66%) 目前顯示: 第 24~46 行 ← 離開",
        "瀏覽 第 2/3 頁 (166%) 目前顯示: 第 24~46 行 ← 離開",
        "瀏覽 第 2/3 頁 (66%) 目前顯示: 第 46~24 行 ← 離開",
        "瀏覽 第 2/3 頁 (66%) 目前顯示: 第 1~100 行 ← 離開",
        "是否繼續？[Y/n]",
    ] {
        let mut screen = fixture("reading_middle");
        replace(&mut screen, 23, status, "30;47");
        screen.feed(b"\x1b[24;80H");
        unknown(&screen.frame());
    }
    let mut screen = fixture("reading_middle");
    screen.feed(b"\x1b[15;20H");
    unknown(&screen.frame());
}
#[test]
fn malformed_frame_geometry_fails_without_panicking_or_reconstructing_cells() {
    let mut frame = fixture("list").frame();
    frame.glyphs.remove(3);
    assert!(text::rows(&frame).is_none());
    unknown(&frame);
    assert!(pttstella::links::detect(&frame).is_empty());
    frame.cols = 81;
    assert_eq!(
        navigation::analyze(&frame),
        Context::Unknown {
            reason: UnknownReason::UnsupportedGrid
        }
    );
}

#[test]
fn menu_board_and_favorite_profiles_translate_without_enabling_keepalive_or_article_hits() {
    for name in ["main_menu", "boards", "favorites"] {
        let context = page(name);
        assert_eq!(
            context.kind(),
            if name == "main_menu" {
                "main_menu"
            } else {
                "board_list"
            }
        );
        assert!(!context.allows_keepalive());
        for (intent, expected) in [
            (
                Intent::Lines {
                    down: true,
                    count: 2,
                },
                b"\x1b[B\x1b[B".as_slice(),
            ),
            (
                Intent::Lines {
                    down: false,
                    count: 1,
                },
                b"\x1b[A",
            ),
            (Intent::First, b"\x1b[H"),
            (Intent::Last, b"\x1b[F"),
            (Intent::Forward, b"\r"),
            (Intent::Back, b"\x1b[D"),
            (Intent::NextPage, b"\x1b[6~"),
            (Intent::PreviousPage, b"\x1b[5~"),
        ] {
            assert_eq!(
                navigation::plan(&context, intent).unwrap(),
                expected,
                "{name}"
            );
        }
        assert_eq!(
            navigation::plan(&context, Intent::Open { row: 4 }),
            Err(Error::Unavailable)
        );
    }
    assert_eq!(page("main_menu").content_rows(), 4);
    assert_eq!(page("boards").content_rows(), 20);
}
#[test]
fn commands_split_between_subheader_and_footer_recognize_article_list() {
    let context = page("list_subheader");
    assert!(matches!(
        context,
        Context::ArticleList { cursor_row: 4, .. }
    ));
    assert!(context.allows_keepalive());
    assert_eq!(navigation::plan(&context, Intent::Forward).unwrap(), b"\r");
    let mut screen = fixture("list_subheader");
    replace(&mut screen, 23, "請輸入密碼:", "30;47");
    screen.feed(b"\x1b[5;2H");
    unknown(&screen.frame());
}
#[test]
fn menu_and_board_profiles_reject_prompts_stale_pointers_and_missing_anchors() {
    for name in ["main_menu", "boards", "favorites"] {
        let cursor = if name == "main_menu" {
            b"\x1b[14;22H".as_slice()
        } else {
            b"\x1b[5;2H"
        };
        for (row, value) in [(23, "是否刪除？[Y/n]"), (2, "請輸入帳號:")] {
            // Main menu has no table header; corrupt its title instead.
            let row = if name == "main_menu" && row == 2 {
                0
            } else {
                row
            };
            let mut screen = fixture(name);
            replace(&mut screen, row, value, "30;47");
            screen.feed(cursor);
            unknown(&screen.frame());
        }
        let mut screen = fixture(name);
        screen.feed(b"\x1b[24;1H");
        unknown(&screen.frame());
        let mut screen = fixture(name);
        screen.feed(if name == "main_menu" {
            b"\x1b[14;21H  "
        } else {
            b"\x1b[5;1H  "
        });
        screen.feed(cursor);
        unknown(&screen.frame());
    }
}

#[test]
fn menu_banner_and_class_decoration_are_not_input_context() {
    for (name, cursor, first_row) in [
        ("main_menu", b"\x1b[14;22H".as_slice(), 12),
        ("classes", b"\x1b[9;2H", 7),
    ] {
        let mut screen = fixture(name);
        screen.feed(b"\x1b[3;1H\x81 ");
        screen.feed(cursor);
        let context = navigation::analyze(&screen.frame());
        assert_eq!(
            context.kind(),
            if name == "main_menu" {
                "main_menu"
            } else {
                "board_list"
            }
        );
        assert_eq!(
            context.content_rows(),
            if name == "main_menu" {
                4
            } else {
                23 - first_row
            }
        );
        assert!(!context.allows_keepalive());
        screen.feed(if name == "main_menu" {
            b"\x1b[14;40H\x81 "
        } else {
            b"\x1b[9;40H\x81 "
        });
        screen.feed(cursor);
        unknown(&screen.frame());
    }
}

#[test]
fn real_board_reading_command_and_full_page_are_a_board_context() {
    let context = page("boards_reading");
    assert_eq!(context, Context::BoardList { first_row: 3 });
    assert_eq!(navigation::plan(&context, Intent::Forward).unwrap(), b"\r");
    assert!(!context.allows_keepalive());
}
#[test]
fn real_dense_recommendations_dates_and_pinned_rows_are_an_article_context() {
    let context = page("list_dense");
    let Context::ArticleList {
        articles,
        cursor_row,
    } = &context
    else {
        panic!("{context:?}")
    };
    assert_eq!(*cursor_row, 17);
    assert_eq!(
        articles.iter().map(|a| a.row).collect::<Vec<_>>(),
        (3..23).collect::<Vec<_>>()
    );
    assert_eq!(navigation::plan(&context, Intent::Forward).unwrap(), b"\r");
}

#[test]
fn legacy_pointer_covering_first_digit_preserves_large_article_navigation() {
    let mut screen = fixture("list_dense");
    let row = article_row("100000", " +16", "10/16", "writer", "□ 合成六位數文章");
    replace(&mut screen, 17, &format!("●{}", &row[2..]), "0");
    screen.feed(b"\x1b[18;2H");
    let context = navigation::analyze(&screen.frame());
    assert!(matches!(
        context,
        Context::ArticleList { cursor_row: 17, .. }
    ));
    assert_eq!(
        navigation::plan(&context, Intent::Open { row: 22 }).unwrap(),
        b"\x1b[B\x1b[B\x1b[B\x1b[B\x1b[B\r"
    );
}
#[test]
fn dense_lists_still_reject_prompt_bad_date_or_missing_forward_command() {
    for name in ["boards_reading", "list_dense"] {
        let mut screen = fixture(name);
        replace(&mut screen, 23, "是否刪除？[Y/n]", "30;47");
        screen.feed(if name == "boards_reading" {
            b"\x1b[4;2H"
        } else {
            b"\x1b[18;2H"
        });
        unknown(&screen.frame());
        let mut screen = fixture(name);
        replace(&mut screen, 1, "[←]離開 [h]說明", "0");
        screen.feed(if name == "boards_reading" {
            b"\x1b[4;2H"
        } else {
            b"\x1b[18;2H"
        });
        unknown(&screen.frame());
    }
    let mut screen = fixture("list_dense");
    screen.feed(b"\x1b[4;12H99/99 \x1b[18;2H");
    unknown(&screen.frame());
}
