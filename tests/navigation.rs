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

#[test]
fn menu_and_favorite_pointer_targets_use_verified_rows_and_exact_relative_input() {
    let mut menu = fixture("main_menu");
    replace(
        &mut menu,
        14,
        "                      (U)ser         【 個人設定區 】",
        "0",
    );
    menu.feed(b"\x1b[14;22H");
    let context = navigation::analyze(&menu.frame());
    assert_eq!(
        context,
        Context::MainMenu {
            first_row: 12,
            last_row: 15,
            cursor_row: 13
        }
    );
    for (row, expected) in [(12, b"\x1b[A\r".as_slice()), (13, b"\r"), (14, b"\x1b[B\r")] {
        assert_eq!(
            navigation::plan(&context, Intent::Open { row }).unwrap(),
            expected
        );
    }
    for name in ["boards", "favorites"] {
        let context = page(name);
        assert_eq!(
            context,
            Context::BoardList {
                first_row: 3,
                last_row: 5,
                cursor_row: 4
            }
        );
        assert_eq!(
            navigation::plan(&context, Intent::Open { row: 3 }).unwrap(),
            b"\x1b[A\r"
        );
        assert_eq!(
            navigation::plan(&context, Intent::Open { row: 4 }).unwrap(),
            b"\r"
        );
        assert_eq!(
            navigation::plan(&context, Intent::Open { row: 5 }).unwrap(),
            b"\x1b[B\r"
        );
        for row in [0, 2, 6, 22, 23, usize::MAX] {
            assert_eq!(
                navigation::plan(&context, Intent::Open { row }),
                Err(Error::Unavailable)
            );
        }
    }
    let invalid = Context::BoardList {
        first_row: 3,
        last_row: usize::MAX,
        cursor_row: 4,
    };
    assert_eq!(
        navigation::plan(&invalid, Intent::Open { row: 4 }),
        Err(Error::Unavailable)
    );
}

#[test]
fn nested_system_menus_and_plain_descriptions_use_verified_rows() {
    for (title, status) in [
        ("私人信件區", "電子郵件"),
        ("個人設定區", "個人設定"),
        ("休閒聊天區", "聊天說話"),
        ("系統資訊區", "工具程式"),
        ("編特別名單", "名單編輯"),
    ] {
        let mut screen = fixture("main_menu");
        replace(&mut screen, 0, title, "37;44");
        replace(
            &mut screen,
            23,
            &format!("{status} (h)說明 (←)回上層"),
            "30;47",
        );
        replace(
            &mut screen,
            14,
            "                      (U)pdate       自製選項",
            "0",
        );
        screen.feed(b"\x1b[14;22H");
        let context = navigation::analyze(&screen.frame());
        assert_eq!(
            context,
            Context::SystemMenu {
                first_row: 12,
                last_row: 15,
                cursor_row: 13
            }
        );
        assert_eq!(
            navigation::plan(&context, Intent::Open { row: 14 }).unwrap(),
            b"\x1b[B\r"
        );
        assert_eq!(navigation::plan(&context, Intent::Back).unwrap(), b"\x1b[D");
        assert!(!context.allows_keepalive());
        replace(&mut screen, 23, "是否確定？[Y/n]", "30;47");
        screen.feed(b"\x1b[14;22H");
        unknown(&screen.frame());
    }
    let mut root = fixture("main_menu");
    replace(
        &mut root,
        15,
        "                      (G)oodbye     自製離開選項",
        "0",
    );
    root.feed(b"\x1b[14;22H");
    assert!(matches!(
        navigation::analyze(&root.frame()),
        Context::MainMenu { last_row: 15, .. }
    ));
}

#[test]
fn favorite_description_glyphs_do_not_decide_navigation_but_names_and_pointer_do() {
    let mut screen = fixture("favorites");
    screen.feed(b"\x1b[4;40H\x81 \x1b[5;2H");
    assert!(screen.frame().glyphs.iter().any(|glyph| glyph.unmapped));
    assert!(matches!(
        navigation::analyze(&screen.frame()),
        Context::BoardList {
            first_row: 3,
            last_row: 5,
            cursor_row: 4
        }
    ));
    screen.feed(b"\x1b[4;11H\x81 \x1b[5;2H");
    unknown(&screen.frame());
}

fn switch_scene(name: &str) -> Vec<u8> {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/board-switch.json")).unwrap();
    encoding::encode(data["screens"][name].as_str().unwrap()).unwrap()
}

#[test]
fn board_names_and_search_prompts_are_bounded_and_exact() {
    use pttstella::board_navigation::{self as board, BoardName};
    for name in ["Stella", "C_Chat", "Ab.c-0_", "a01234567890"] {
        assert_eq!(BoardName::parse(name).unwrap().as_str(), name);
    }
    assert_eq!(BoardName::parse(" Stella ").unwrap().as_str(), "Stella");
    for name in [
        "",
        "a",
        "1abc",
        "中文",
        "a123456789012",
        "bad\rname",
        "abc def",
        "a/b",
        "a\\b",
    ] {
        assert!(BoardName::parse(name).is_err(), "{name:?}");
    }
    let bytes = switch_scene("prompt");
    for split in 0..=bytes.len() {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&bytes[..split]);
        screen.feed(&bytes[split..]);
        assert!(board::search_prompt(&screen.frame()));
    }
    for suffix in [b"x".as_slice(), b"\x1b[", b"\x1b[?2026h", b"\x1b[3;1H"] {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&bytes);
        screen.feed(suffix);
        assert!(!board::search_prompt(&screen.frame()));
    }
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&bytes);
    let (col, row) = screen.cursor();
    screen.feed(
        &encoding::encode(&format!(
            "\x1b[H\x1b[0m【 搜尋全站看板 】\x1b[{};{}H",
            row + 1,
            col + 1
        ))
        .unwrap(),
    );
    assert!(!board::search_prompt(&screen.frame()));
}

#[test]
fn board_identity_uses_the_actual_header_suffix_and_distinguishes_empty_lists_and_folders() {
    use pttstella::board_navigation as board;
    for (name, browser, current, selected) in [
        ("boards", true, None, Some("Alpha")),
        ("selected", true, None, Some("Stella")),
        ("empty_boards", true, None, None),
        ("articles", false, Some("Stella"), None),
        ("empty_articles", false, Some("Stella"), None),
        ("reading", false, None, None),
    ] {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&switch_scene(name));
        assert_eq!(board::browser(&screen.frame()), browser, "{name}");
        assert_eq!(
            board::current_board(&screen.frame())
                .as_ref()
                .map(|name| name.as_str()),
            current,
            "{name}"
        );
        assert_eq!(
            board::selected_board(&screen.frame())
                .as_ref()
                .map(|name| name.as_str()),
            selected,
            "{name}"
        );
    }
    assert_eq!(
        board::current_board(&fixture("list").frame())
            .unwrap()
            .as_str(),
        "Stella"
    );
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&switch_scene("articles"));
    screen.feed(
        &encoding::encode(
            "\x1b[H\x1b[37;44m\x1b[2K描述中的看板《Other》 模擬敘述 看板《Stella》\x1b[0m\x1b[5;1H",
        )
        .unwrap(),
    );
    assert_eq!(
        board::current_board(&screen.frame()).unwrap().as_str(),
        "Stella"
    );
    screen.feed(&switch_scene("selected"));
    screen.feed(&encoding::encode("\x1b[5;24H目錄\x1b[5;1H").unwrap());
    assert!(board::browser(&screen.frame()));
    assert!(board::selected_board(&screen.frame()).is_none());
}

#[test]
fn switch_from_main_waits_for_prompt_then_selected_target_before_entering_once() {
    use pttstella::board_navigation::{Advance, BoardName, Start, Switch};
    let now = std::time::Instant::now();
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&switch_scene("main"));
    let Start::Run { mut switch, bytes } =
        Switch::begin(&screen.frame(), BoardName::parse("stella").unwrap(), now).unwrap()
    else {
        panic!("expected workflow")
    };
    assert_eq!(bytes, b"f");
    assert_eq!(switch.observe(&screen.frame(), now).unwrap(), Advance::Wait);
    for (name, expected) in [
        ("boards", b"s".as_slice()),
        ("prompt", b"stella\r"),
        ("selected", b"\x1b[C"),
    ] {
        screen.feed(&switch_scene(name));
        assert_eq!(
            switch.observe(&screen.frame(), now).unwrap(),
            Advance::Send(expected.to_vec())
        );
        assert_eq!(switch.observe(&screen.frame(), now).unwrap(), Advance::Wait);
    }
    screen.feed(&switch_scene("articles"));
    assert_eq!(
        switch.observe(&screen.frame(), now).unwrap(),
        Advance::Reached(BoardName::parse("Stella").unwrap())
    );
}

#[test]
fn switch_from_reading_waits_for_each_back_step_and_accepts_direct_or_empty_target_arrival() {
    use pttstella::board_navigation::{Advance, BoardName, Start, Switch};
    for destination in ["articles", "empty_articles"] {
        let now = std::time::Instant::now();
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&switch_scene("reading"));
        let Start::Run { mut switch, bytes } =
            Switch::begin(&screen.frame(), BoardName::parse("Stella").unwrap(), now).unwrap()
        else {
            panic!("expected workflow")
        };
        assert_eq!(bytes, b"\x1b[D");
        for (name, expected) in [
            ("other_articles", b"\x1b[D".as_slice()),
            ("empty_boards", b"s"),
            ("prompt", b"Stella\r"),
        ] {
            screen.feed(&switch_scene(name));
            assert_eq!(
                switch.observe(&screen.frame(), now).unwrap(),
                Advance::Send(expected.to_vec())
            );
        }
        screen.feed(&switch_scene(destination));
        assert_eq!(
            switch.observe(&screen.frame(), now).unwrap(),
            Advance::Reached(BoardName::parse("Stella").unwrap())
        );
        assert!(
            matches!(Switch::begin(&screen.frame(),BoardName::parse("stella").unwrap(),now),Ok(Start::Reached(name)) if name.as_str()=="Stella")
        );
    }
}

#[test]
fn switch_never_answers_entry_or_login_prompts_and_partial_or_wrong_frames_do_not_extend_waits() {
    use pttstella::board_navigation::{self as board, Advance, BoardName, Start, Switch};
    use std::time::{Duration, Instant};
    let now = Instant::now();
    for name in ["entry", "help", "prompt"] {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&switch_scene(name));
        assert!(matches!(
            Switch::begin(&screen.frame(), BoardName::parse("Stella").unwrap(), now),
            Err(board::Error::Unsupported)
        ));
    }
    for name in ["entry", "help", "login"] {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&switch_scene("boards"));
        let Start::Run { mut switch, .. } =
            Switch::begin(&screen.frame(), BoardName::parse("Stella").unwrap(), now).unwrap()
        else {
            panic!("workflow")
        };
        if name == "login" {
            screen.feed(
                &encoding::encode("\x1b[2J\x1b[21;1H請輸入代號，或以 guest 參觀，或以 new 註冊: ")
                    .unwrap(),
            );
        } else {
            screen.feed(&switch_scene(name));
        }
        assert_eq!(
            switch.observe(&screen.frame(), now),
            Err(board::Error::Manual)
        );
    }
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&switch_scene("boards"));
    let Start::Run { mut switch, .. } =
        Switch::begin(&screen.frame(), BoardName::parse("Stella").unwrap(), now).unwrap()
    else {
        panic!("workflow")
    };
    screen.feed(b"\x1b[?2026h");
    screen.feed(&switch_scene("prompt"));
    assert_eq!(switch.observe(&screen.frame(), now).unwrap(), Advance::Wait);
    screen.feed(b"\x1b[?2026l");
    assert_eq!(
        switch.observe(&screen.frame(), now).unwrap(),
        Advance::Send(b"Stella\r".to_vec())
    );
    for elapsed in [1, 2, 3, 4] {
        screen.feed(&switch_scene("boards")); // Still points at Alpha, not Stella.
        assert_eq!(
            switch
                .observe(&screen.frame(), now + Duration::from_secs(elapsed))
                .unwrap(),
            Advance::Wait
        );
    }
    assert_eq!(
        switch.observe(&screen.frame(), now + board::STEP_LIMIT),
        Err(board::Error::Timeout)
    );
}
fn page(name: &str) -> Context {
    navigation::analyze(&fixture(name).frame())
}
fn action_fixture(name: &str) -> Screen {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/navigation-actions.json")).unwrap();
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&encoding::encode(data["screens"][name].as_str().unwrap()).unwrap());
    screen
}

#[test]
fn dynamic_action_bars_preserve_existing_list_menu_and_reading_contracts() {
    for (name, kind) in [
        ("boards", "board_list"),
        ("favorites", "board_list"),
        ("articles", "article_list"),
        ("main", "main_menu"),
        ("reading", "article_reading"),
    ] {
        let screen = action_fixture(name);
        let context = navigation::analyze(&screen.frame());
        assert_eq!(context.kind(), kind, "{name}: {context:?}");
        assert_eq!(navigation::plan(&context, Intent::Back).unwrap(), b"\x1b[D");
        let mut wrong = screen.clone();
        wrong.feed(b"\x1b[1;1H");
        unknown(&wrong.frame());
        let mut partial = screen;
        partial.feed(b"\x1b[?2026h");
        unknown(&partial.frame());
    }
}

#[test]
fn command_help_is_a_separate_profile_that_cannot_execute_a_command_by_browsing() {
    let context = navigation::analyze(&action_fixture("help").frame());
    assert_eq!(
        context,
        Context::CommandHelp {
            first_row: 2,
            last_row: 4
        }
    );
    assert!(!context.allows_keepalive());
    for (intent, bytes) in [
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
        (Intent::PreviousPage, b"\x1b[5~"),
        (Intent::NextPage, b"\x1b[6~"),
        (Intent::Back, b"\x1b[D"),
    ] {
        assert_eq!(navigation::plan(&context, intent).unwrap(), bytes);
    }
    for intent in [Intent::Forward, Intent::Open { row: 3 }] {
        assert_eq!(
            navigation::plan(&context, intent),
            Err(Error::RequiresNativeInput)
        );
    }
    for edit in [
        "\x1b[1;1H",                 // Cursor outside the recognized items.
        "\x1b[3;1H \x1b[3;1H",       // Missing selection pointer.
        "\x1b[4;1H\x1b[2K\x1b[3;1H", // Gap before another item.
        "\x1b[4;1H請輸入您的密碼: \x1b[3;1H",
        "\x1b[24;1H\x1b[2K是否執行？(y/N)\x1b[3;1H",
        "\x1b[2;1H\x1b[0m按鍵 / 標籤                   功能說明\x1b[3;1H",
    ] {
        let mut screen = action_fixture("help");
        screen.feed(&encoding::encode(edit).unwrap());
        unknown(&screen.frame());
    }
    let mut unmapped = action_fixture("help");
    unmapped.feed(b"\x1b[3;1H\xa4 \x1b[3;1H");
    unknown(&unmapped.frame());
    let mut article = fixture("reading_middle");
    replace(
        &mut article,
        8,
        "操作說明 (Help) 按鍵 / 標籤 功能說明",
        "37;44",
    );
    article.feed(b"\x1b[24;80H");
    assert!(matches!(
        navigation::analyze(&article.frame()),
        Context::ArticleReading { .. }
    ));
}

#[test]
fn empty_lists_only_admit_back_with_exact_empty_body_and_bottom_cursor() {
    for name in ["empty_boards", "empty_articles"] {
        let screen = action_fixture(name);
        let context = navigation::analyze(&screen.frame());
        assert_eq!(context, Context::EmptyList, "{name}");
        assert!(!context.allows_keepalive());
        assert_eq!(navigation::plan(&context, Intent::Back).unwrap(), b"\x1b[D");
        for intent in [
            Intent::Forward,
            Intent::NextPage,
            Intent::First,
            Intent::Open { row: 3 },
            Intent::Lines {
                down: true,
                count: 1,
            },
        ] {
            assert_eq!(navigation::plan(&context, intent), Err(Error::Unavailable));
        }
        for edit in [
            "\x1b[4;1H",
            "\x1b[5;1Hother content\x1b[24;80H",
            "\x1b[2;1H請輸入您的密碼: \x1b[24;80H",
        ] {
            let mut invalid = screen.clone();
            invalid.feed(&encoding::encode(edit).unwrap());
            unknown(&invalid.frame());
        }
    }
}
fn unknown(frame: &Frame) {
    assert!(matches!(
        navigation::analyze(frame),
        Context::Unknown { .. }
    ));
}

#[test]
fn ascii_pointer_touching_six_digit_number_is_a_valid_selected_article() {
    let mut screen = fixture("list_dense");
    // Restore the old legacy marker's covered digit, then select a dense row
    // using the normal one-cell marker. No separating blank follows the >.
    screen.feed(b"\x1b[18;1H 3\x1b[4;1H>\x1b[4;1H");
    let context = navigation::analyze(&screen.frame());
    assert!(
        matches!(context, Context::ArticleList { cursor_row: 3, .. }),
        "{context:?}"
    );
    assert_eq!(navigation::plan(&context, Intent::Forward).unwrap(), b"\r");
    assert_eq!(
        navigation::plan(&context, Intent::Open { row: 4 }).unwrap(),
        b"\x1b[B\r"
    );
    screen.feed(b"\x1b[4;1H \x1b[4;1H");
    unknown(&screen.frame());
    screen.feed(b"\x1b[4;1Hx\x1b[4;1H");
    unknown(&screen.frame());
}

#[test]
fn board_entry_pause_uses_only_the_complete_styled_footer() {
    let mut screen = fixture("board_entry");
    assert_eq!(screen.frame().cursor, (79, 23));
    assert_eq!(
        navigation::analyze(&screen.frame()),
        Context::ContinuePrompt
    );
    // Unmapped artwork outside the decision region must not suspend the prompt.
    screen.feed(b"\x1b[4;1H\xa4 \x1b[24;80H");
    assert!(screen.frame().glyphs.iter().any(|g| g.unmapped));
    let context = navigation::analyze(&screen.frame());
    assert_eq!(context, Context::ContinuePrompt);
    assert_eq!(navigation::plan(&context, Intent::Continue).unwrap(), b" ");
    assert!(!context.allows_keepalive());
    for intent in [
        Intent::Forward,
        Intent::Back,
        Intent::NextPage,
        Intent::First,
    ] {
        assert_eq!(navigation::plan(&context, intent), Err(Error::Unavailable));
    }
    for name in ["list", "reading_middle", "main_menu", "favorites"] {
        assert_eq!(
            navigation::plan(&page(name), Intent::Continue),
            Err(Error::Unavailable)
        );
    }
}

#[test]
fn pause_text_in_body_partial_footer_or_other_prompt_is_not_admitted() {
    for edit in [
        "\x1b[8;1H", // body cursor, even with the old pause footer present
        "\x1b[24;1H\x1b[0m\x1b[2K請按任意鍵繼續\x1b[24;80H",
        "\x1b[24;1H\x1b[37;44m\x1b[2K是否繼續？(y/N)\x1b[24;80H",
        "\x1b[24;1H\x1b[37;44m\x1b[2K請按任意鍵繼續 請輸入密碼:\x1b[24;80H",
        "\x1b[24;1H\x1b[37;44mX\x1b[24;80H",
        "\x1b[31;", // incomplete control sequence
    ] {
        let mut screen = fixture("board_entry");
        screen.feed(&encoding::encode(edit).unwrap());
        unknown(&screen.frame());
    }
    let mut body = fixture("reading_middle");
    body.feed(&encoding::encode("\x1b[8;1H\x1b[37;44m請按任意鍵繼續\x1b[0m\x1b[24;80H").unwrap());
    assert!(matches!(
        navigation::analyze(&body.frame()),
        Context::ArticleReading { .. }
    ));
    let mut footer = fixture("board_entry");
    footer.feed(b"\x1b[24;1H\xa4 \x1b[24;80H");
    unknown(&footer.frame());
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
fn live_empty_article_titles_keep_valid_number_and_author_targets() {
    let mut screen = fixture("list_dense");
    replace(
        &mut screen,
        14,
        &article_row("774", "    ", "10/16", "empty_writer", "□"),
        "0",
    );
    screen.feed(b"\x1b[18;2H");
    let context = navigation::analyze(&screen.frame());
    assert!(
        matches!(context, Context::ArticleList { .. }),
        "{context:?}"
    );
    assert_eq!(
        navigation::plan(&context, Intent::Open { row: 14 }).unwrap(),
        b"\x1b[A\x1b[A\x1b[A\r"
    );
}
#[test]
fn live_important_pinned_date_is_not_an_invalid_calendar_date() {
    let mut screen = fixture("list_dense");
    replace(
        &mut screen,
        19,
        &article_row("★", " !  ", "重要 ", "announcer", "□ 自製重要公告"),
        "0",
    );
    screen.feed(b"\x1b[18;2H");
    let context = navigation::analyze(&screen.frame());
    assert!(
        matches!(context, Context::ArticleList { .. }),
        "{context:?}"
    );
    for (number, date) in [("774", "重要 "), ("★", "未知 "), ("774", "99/99")] {
        replace(
            &mut screen,
            19,
            &article_row(number, "    ", date, "announcer", "□ 自製無效日期"),
            "0",
        );
        screen.feed(b"\x1b[18;2H");
        unknown(&screen.frame());
    }
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
fn reply_command_footer_keeps_all_ten_reading_pages_navigable() {
    for (index, percent) in [9, 18, 26, 35, 44, 52, 64, 74, 86, 100]
        .into_iter()
        .enumerate()
    {
        let first = 1 + index * 22;
        let mut screen = fixture("reading_middle");
        replace(
            &mut screen,
            23,
            &format!(
                "瀏覽 第 {}/10 頁 ({percent}%) 目前顯示: 第 {first}~{} 行 (y)回應(X)推文(h)說明",
                index + 1,
                first + 22
            ),
            "30;47",
        );
        screen.feed(b"\x1b[24;80H");
        let context = navigation::analyze(&screen.frame());
        assert_eq!(
            context,
            Context::ArticleReading {
                first_line: first,
                last_line: first + 22,
                percent
            }
        );
        assert_eq!(
            navigation::plan(&context, Intent::NextPage),
            if percent == 100 {
                Err(Error::Unavailable)
            } else {
                Ok(b"\x1b[6~".to_vec())
            }
        );
        assert_eq!(
            navigation::plan(&context, Intent::PreviousPage),
            if first == 1 {
                Err(Error::Unavailable)
            } else {
                Ok(b"\x1b[5~".to_vec())
            }
        );
        assert_eq!(navigation::plan(&context, Intent::Back).unwrap(), b"\x1b[D");
    }
}

#[test]
fn reply_footer_requires_the_complete_styled_command_band_and_range() {
    let report = "瀏覽 第 6/10 頁 (52%) 目前顯示: 第 110~132 行";
    for (suffix, sgr) in [
        ("(y)回應(X)推文(h)說明", "0"),
        ("(n)回應(X)推文(h)說明", "30;47"),
        ("(y)回應(x)推文(h)說明", "30;47"),
        ("(y)回應(X)推文", "30;47"),
        ("是否執行？(y)回應(X)推文(h)說明", "30;47"),
        ("(y)回應(X)推文(h)說明 請輸入密碼:", "30;47"),
    ] {
        let mut screen = fixture("reading_middle");
        replace(&mut screen, 23, &format!("{report} {suffix}"), sgr);
        screen.feed(b"\x1b[24;80H");
        unknown(&screen.frame());
    }
    for edit in [
        "\x1b[4;1H",
        "\x1b[24;30H\x1b[0m目前顯示\x1b[24;80H",
        "\x1b[31;",
    ] {
        let mut screen = fixture("reading_middle");
        replace(
            &mut screen,
            23,
            &format!("{report} (y)回應 (X)推文 (h)說明"),
            "30;47",
        );
        screen.feed(b"\x1b[24;80H");
        screen.feed(&encoding::encode(edit).unwrap());
        unknown(&screen.frame());
    }
    let mut screen = fixture("reading_middle");
    replace(
        &mut screen,
        23,
        &format!("{report} (y)回應(X)推文(h)說明"),
        "30;47",
    );
    let frame = screen.frame();
    let rows = text::rows(&frame).unwrap();
    let col = rows[23]
        .spans
        .iter()
        .find(|s| s.glyph.text == "X")
        .unwrap()
        .glyph
        .col;
    screen.feed(format!("\x1b[24;{}H\x1b[0mX\x1b[24;80H", col + 1).as_bytes());
    unknown(&screen.frame());
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
            navigation::plan(&context, Intent::Open { row: 23 }),
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
    assert_eq!(
        context,
        Context::BoardList {
            first_row: 3,
            last_row: 22,
            cursor_row: 3
        }
    );
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

#[test]
fn board_description_caption_does_not_disable_frequent_board_switching() {
    let context = page("boards_caption");
    assert_eq!(
        context,
        Context::BoardList {
            first_row: 3,
            last_row: 22,
            cursor_row: 6
        }
    );
    for (intent, expected) in [
        (
            Intent::Lines {
                down: true,
                count: 3,
            },
            b"\x1b[B\x1b[B\x1b[B".as_slice(),
        ),
        (Intent::Forward, b"\r"),
        (Intent::Back, b"\x1b[D"),
    ] {
        assert_eq!(navigation::plan(&context, intent).unwrap(), expected);
    }
    assert!(!context.allows_keepalive());
}

#[test]
fn board_browser_still_requires_stable_columns_and_valid_rows() {
    for caption in ["中文敘述", "中文測述", "描述", ""] {
        let mut screen = fixture("boards_caption");
        replace(
            &mut screen,
            2,
            &format!("編號 看板 類別 {caption} 人氣 板主"),
            "30;47",
        );
        screen.feed(b"\x1b[7;2H");
        assert_eq!(
            navigation::analyze(&screen.frame()),
            Context::BoardList {
                first_row: 3,
                last_row: 22,
                cursor_row: 6
            }
        );
    }
    for header in [
        "編號 看板 類別 中文測述 板主",
        "編號 看板 類別 中文測述 人氣",
    ] {
        let mut screen = fixture("boards_caption");
        replace(&mut screen, 2, header, "30;47");
        screen.feed(b"\x1b[7;2H");
        assert_eq!(
            navigation::analyze(&screen.frame()),
            Context::Unknown {
                reason: UnknownReason::BoardHeader
            }
        );
    }
    let mut screen = fixture("boards_caption");
    replace(&mut screen, 4, "          是否刪除？", "0");
    screen.feed(b"\x1b[7;2H");
    assert_eq!(
        navigation::analyze(&screen.frame()),
        Context::Unknown {
            reason: UnknownReason::BoardRows
        }
    );
}

#[test]
fn mail_and_archive_lists_use_verified_single_column_rows_without_new_mail_shortcuts() {
    use navigation::DocumentKind;
    for (name, category) in [
        ("mail", DocumentKind::Mail),
        ("archive", DocumentKind::Archive),
    ] {
        let mut screen = fixture(name);
        let context = navigation::analyze(&screen.frame());
        assert_eq!(
            context,
            Context::DocumentList {
                category,
                first_row: 3,
                last_row: 4,
                cursor_row: 4
            }
        );
        assert!(!context.allows_keepalive());
        for (intent, expected) in [
            (Intent::Open { row: 3 }, b"\x1b[A\r".as_slice()),
            (Intent::Open { row: 4 }, b"\r"),
            (Intent::Back, b"\x1b[D"),
            (Intent::NextPage, b"\x1b[6~"),
            (Intent::PreviousPage, b"\x1b[5~"),
        ] {
            assert_eq!(navigation::plan(&context, intent).unwrap(), expected);
        }
        for row in [0, 2, 5, 23, usize::MAX] {
            assert_eq!(
                navigation::plan(&context, Intent::Open { row }),
                Err(Error::Unavailable)
            );
        }
        replace(&mut screen, 23, "您確定要刪除嗎 [Y/N]", "0");
        assert!(matches!(
            navigation::analyze(&screen.frame()),
            Context::Unknown { .. }
        ));
    }
}
#[test]
fn document_list_rejects_multi_column_headers_missing_rows_and_cursor_mismatch() {
    let mut screen = fixture("mail");
    replace(
        &mut screen,
        2,
        "編號 日期 作者 信件標題       編號 日期 作者 信件標題",
        "30;47",
    );
    assert_eq!(
        navigation::analyze(&screen.frame()),
        Context::Unknown {
            reason: UnknownReason::UnsupportedLayout
        }
    );
    let mut screen = fixture("archive");
    replace(&mut screen, 3, "", "0");
    assert_eq!(
        navigation::analyze(&screen.frame()),
        Context::Unknown {
            reason: UnknownReason::Incomplete
        }
    );
    let mut screen = fixture("mail");
    screen.feed(b"\x1b[20;1H");
    assert_eq!(
        navigation::analyze(&screen.frame()),
        Context::Unknown {
            reason: UnknownReason::InvalidCursor
        }
    );
}

#[test]
fn modern_board_captions_and_indented_class_pointer_keep_required_item_boundaries() {
    for name in ["favorites_modern", "boards_modern"] {
        let mut screen = fixture(name);
        let context = navigation::analyze(&screen.frame());
        assert_eq!(
            context,
            Context::BoardList {
                first_row: 3,
                last_row: 5,
                cursor_row: 4
            },
            "{name}"
        );
        assert_eq!(
            navigation::plan(&context, Intent::Open { row: 3 }).unwrap(),
            b"\x1b[A\r"
        );
        replace(&mut screen, 23, "是否離開 [Y/N]", "0");
        unknown(&screen.frame());
    }
    let mut screen = fixture("classes_indented");
    let context = navigation::analyze(&screen.frame());
    assert_eq!(
        context,
        Context::BoardList {
            first_row: 7,
            last_row: 8,
            cursor_row: 8
        }
    );
    assert_eq!(
        navigation::plan(&context, Intent::Open { row: 7 }).unwrap(),
        b"\x1b[A\r"
    );
    screen.feed(b"\x1b[9;20H");
    unknown(&screen.frame());
}

#[test]
fn root_board_action_bars_admit_only_their_specific_missing_arrow_layouts() {
    for (name, first_row, last_row, cursor_row) in [
        ("favorites_root_actions", 3, 5, 4),
        ("classes_root_actions", 7, 8, 8),
    ] {
        let context = page(name);
        assert_eq!(
            context,
            Context::BoardList {
                first_row,
                last_row,
                cursor_row
            },
            "{name}"
        );
        assert_eq!(
            navigation::plan(
                &context,
                Intent::Open {
                    row: cursor_row - 1
                }
            )
            .unwrap(),
            b"\x1b[A\r"
        );
        assert_eq!(navigation::plan(&context, Intent::Back).unwrap(), b"\x1b[D");
        assert_eq!(
            navigation::plan(&context, Intent::NextPage).unwrap(),
            b"\x1b[6~"
        );
        assert!(!context.allows_keepalive());
        for (row, text) in [(23, "是否確定？[Y/n]"), (0, "未知選單")] {
            let mut screen = fixture(name);
            replace(&mut screen, row, text, "30;47");
            screen.feed(if first_row == 3 {
                b"\x1b[5;2H"
            } else {
                b"\x1b[9;11H"
            });
            unknown(&screen.frame());
        }
        let mut screen = fixture(name);
        screen.feed(b"\x1b[24;80H");
        unknown(&screen.frame());
        let mut screen = fixture(name);
        let cursor = if first_row == 3 {
            b"\x1b[5;2H".as_slice()
        } else {
            b"\x1b[9;11H"
        };
        let footer = text::rows(&screen.frame()).unwrap()[23].text.clone();
        replace(&mut screen, 23, &footer, "0");
        screen.feed(cursor);
        unknown(&screen.frame());
        let mut screen = fixture(name);
        screen.feed(if first_row == 3 {
            b"\x1b[5;1H> \x1b[5;1H"
        } else {
            b"\x1b[9;11H> \x1b[9;11H"
        });
        assert_eq!(navigation::analyze(&screen.frame()), context);
    }
    for footer in [
        "我的最愛 (a)增加看板 (d)刪除 (g)新增目錄 (M)移動位置 (v)已讀/未讀",
        "我的最愛 (a)增加看板 (d)刪除 (g)新增目錄 (v)已讀/未讀 (h)說明",
        "看板列表 (a)增加看板 (d)刪除 (g)新增目錄 (M)移動位置 (v)已讀/未讀 (h)說明",
        "我的最愛 (a)增加看板 (d)刪除 (g)新增目錄 (M)移動位置 (y)已讀/未讀 (h)說明",
    ] {
        let mut screen = fixture("favorites_root_actions");
        replace(&mut screen, 23, footer, "30;47");
        screen.feed(b"\x1b[5;2H");
        unknown(&screen.frame());
    }
    let mut screen = fixture("favorites_root_actions");
    replace(&mut screen, 1, "[→]進入 [s]找看板 [/]搜尋", "0");
    screen.feed(b"\x1b[5;2H");
    unknown(&screen.frame());
    let mut screen = fixture("classes_root_actions");
    replace(
        &mut screen,
        23,
        "分類看板 (m)加入最愛 (v)已讀/未讀 (t)標記 (y)只列最愛",
        "30;47",
    );
    screen.feed(b"\x1b[9;11H");
    unknown(&screen.frame());
    let mut screen = fixture("classes_root_actions");
    replace(
        &mut screen,
        23,
        "分類看板 (m)加入最愛 (y)已讀/未讀 (t)標記 (v)只列最愛 (←)回上層",
        "30;47",
    );
    screen.feed(b"\x1b[9;11H");
    unknown(&screen.frame());
}
