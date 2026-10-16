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
