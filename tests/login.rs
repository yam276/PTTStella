use pttstella::{
    encoding,
    login::{self, Credentials, Prompt},
    screen::Screen,
};
use zeroize::Zeroizing;
fn input(row: usize, text: &str) -> Screen {
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&encoding::encode(&format!("\x1b[{};1H{}", row + 1, text)).unwrap());
    screen
}
#[test]
fn only_empty_ptt_prompts_at_the_fixed_input_rows_accept_credentials() {
    let account = "請輸入代號，或以 guest 參觀，或以 new 註冊: ";
    assert_eq!(
        login::prompt(&input(20, account).frame()),
        Some(Prompt::Account)
    );
    assert_eq!(
        login::prompt(&input(21, "請輸入您的密碼: ").frame()),
        Some(Prompt::Password)
    );
    for (row, text) in [
        (19, account),
        (20, "請輸入您的密碼: "),
        (21, "文章內容：請輸入您的密碼: "),
        (21, "請設定您的密碼: "),
        (20, "請輸入代號，或以 guest 參觀，或以 new 註冊: Stella"),
    ] {
        assert_eq!(login::prompt(&input(row, text).frame()), None);
    }
    let mut screen = input(21, "請輸入您的密碼: ");
    screen.feed(b"\x1b[");
    assert_eq!(login::prompt(&screen.frame()), None);
    let mut screen = input(21, "請輸入您的密碼: ");
    screen.feed(b"\x81 ");
    assert_eq!(login::prompt(&screen.frame()), None);
}
#[test]
fn credentials_preserve_printable_passwords_and_reject_control_injection() {
    let c = Credentials::new(
        "StellaTest".into(),
        Zeroizing::new(b" synthetic+42 ".to_vec()),
    )
    .unwrap();
    assert_eq!(&*c.account_input(), b"StellaTest\r");
    assert_eq!(&*c.password_input(), b" synthetic+42 \r");
    for account in [
        "guest",
        "new",
        "a",
        "1user",
        "bad\rname",
        "Stella_Test",
        "abcdefghijklmn",
    ] {
        assert!(Credentials::new(account.into(), Zeroizing::new(b"valid".to_vec())).is_err());
    }
    for password in [
        b"".as_slice(),
        b"abc\rdef",
        b"abc\x1bdef",
        b"abc\0def",
        b"abc\x7fdef",
        &[b'a'; 73],
    ] {
        assert!(Credentials::new("StellaTest".into(), Zeroizing::new(password.to_vec())).is_err());
    }
    assert!(Credentials::new("StellaTest".into(), Zeroizing::new(vec![b'x'; 72])).is_ok());
}

#[test]
fn password_requires_the_account_echo_and_rejects_browser_header() {
    let mut screen = input(20, "請輸入代號，或以 guest 參觀，或以 new 註冊: StellaTest");
    screen.feed(&encoding::encode("\x1b[22;1H請輸入您的密碼: ").unwrap());
    assert!(login::password_prompt(&screen.frame(), "StellaTest"));
    assert!(!login::password_prompt(&screen.frame(), "OtherTest"));
    screen.feed(&encoding::encode("\x1b[1;1H\x1b[37;44m【看板列表】\x1b[0m\x1b[22;19H").unwrap());
    assert!(!login::password_prompt(&screen.frame(), "StellaTest"));
}

const CHALLENGE: &str = "請輸入兩階段(2FA)驗證碼(6位限時數字 或8位救援碼): ";

#[test]
fn verification_requires_a_complete_empty_fixed_row_and_rejects_article_content() {
    use login::Guidance::{CodeRejected, CodeRequired};
    for spacing in [CHALLENGE.to_owned(), CHALLENGE.replace("或8", "或 8")] {
        let bytes = encoding::encode(&format!("\x1b[22;1H{spacing}")).unwrap();
        for split in 0..=bytes.len() {
            let mut screen = Screen::new(80, 24).unwrap();
            screen.feed(&bytes[..split]);
            screen.feed(&bytes[split..]);
            assert_eq!(login::verification(&screen.frame()), Some(CodeRequired));
        }
    }
    for (row, message) in [
        (20, "驗證碼錯誤！請確認後重新輸入。"),
        (22, "驗證碼錯誤。請確定在時限前輸入完畢。"),
    ] {
        let mut screen = input(row, message);
        screen.feed(&encoding::encode(&format!("\x1b[22;1H{CHALLENGE}")).unwrap());
        assert_eq!(login::verification(&screen.frame()), Some(CodeRejected));
    }
    for (row, text) in [
        (20, CHALLENGE.to_owned()),
        (21, format!("文章：{CHALLENGE}")),
        (21, format!("{CHALLENGE}123456")),
        (21, format!("{CHALLENGE}ABCDEFGH")),
    ] {
        assert_eq!(login::verification(&input(row, &text).frame()), None);
    }
    for trailer in [b"\x1b[".as_slice(), b"\x1b[?2026h", b"\x81 "] {
        let mut screen = input(21, CHALLENGE);
        screen.feed(trailer);
        assert_eq!(login::verification(&screen.frame()), None);
    }
    let mut screen = input(21, CHALLENGE);
    let cursor = screen.frame().cursor;
    screen.feed(
        &encoding::encode(&format!(
            "\x1b[3;1H\x1b[37;44m作者\x1b[0m\x1b[22;{}H",
            cursor.0 + 1
        ))
        .unwrap(),
    );
    assert_eq!(login::verification(&screen.frame()), None);
}

#[test]
fn followup_is_bounded_and_only_tracks_one_explicit_attempt_without_secrets() {
    use login::{FollowUp, Guidance::*};
    use std::time::{Duration, Instant};
    let now = Instant::now();
    let mut screen = input(21, "請輸入您的密碼: ");
    let mut followup = FollowUp::after_password(screen.revision(), now);
    assert_eq!(followup.observe(&screen.frame(), now), None);
    screen.feed(&encoding::encode(&format!("\x1b[22;1H\x1b[K{CHALLENGE}")).unwrap());
    assert_eq!(followup.observe(&screen.frame(), now), Some(CodeRequired));
    assert_eq!(followup.observe(&screen.frame(), now), None);
    assert_eq!(
        followup.submitted_code(screen.revision()),
        Some(CheckingCode)
    );
    assert_eq!(followup.submitted_code(screen.revision()), None);
    screen.feed(
        &encoding::encode(&format!(
            "\x1b[21;1H驗證碼錯誤！請確認後重新輸入。\x1b[22;1H{CHALLENGE}"
        ))
        .unwrap(),
    );
    assert_eq!(followup.observe(&screen.frame(), now), Some(CodeRejected));
    assert_eq!(
        followup.submitted_code(screen.revision()),
        Some(CheckingCode)
    );
    screen.feed(
        &encoding::encode("\x1b[2J\x1b[21;1H請輸入代號，或以 guest 參觀，或以 new 註冊: ").unwrap(),
    );
    assert_eq!(followup.observe(&screen.frame(), now), Some(Manual));

    let mut followup = FollowUp::after_password(screen.revision(), now);
    screen.feed(b"\x1b[");
    assert_eq!(
        followup.observe(
            &screen.frame(),
            now + login::FOLLOWUP_LIMIT - Duration::from_nanos(1)
        ),
        None
    );
    assert_eq!(
        followup.observe(&screen.frame(), now + login::FOLLOWUP_LIMIT),
        Some(Expired)
    );
    assert_eq!(
        followup.observe(&screen.frame(), now + login::FOLLOWUP_LIMIT),
        None
    );
}
