//! Explicit, single-attempt login. Secrets have no Debug, Clone or Serialize path.
use crate::{screen::Frame, text};
use zeroize::Zeroizing;

#[cfg(feature = "desktop")]
pub mod vault;

pub struct Credentials {
    pub account: String,
    password: Zeroizing<Vec<u8>>,
}
impl Credentials {
    pub fn new(account: String, password: Zeroizing<Vec<u8>>) -> Result<Self, &'static str> {
        if !(2..=12).contains(&account.len())
            || !account.as_bytes()[0].is_ascii_alphabetic()
            || !account.bytes().all(|b| b.is_ascii_alphanumeric())
            || ["guest", "new", "register", "recover"]
                .contains(&account.to_ascii_lowercase().as_str())
        {
            return Err("帳號須為 2–12 個英數字、英文開頭，且不可為訪客或註冊指令");
        }
        if !(1..=72).contains(&password.len()) || !password.iter().all(|b| (32..=126).contains(b)) {
            return Err("密碼須為 1–72 個可列印的英數符號，不可包含換行或控制碼");
        }
        Ok(Self { account, password })
    }
    pub fn account_input(&self) -> Zeroizing<Vec<u8>> {
        let mut bytes = Zeroizing::new(self.account.as_bytes().to_vec());
        bytes.push(b'\r');
        bytes
    }
    pub fn password_input(&self) -> Zeroizing<Vec<u8>> {
        let mut bytes = Zeroizing::new(self.password.to_vec());
        bytes.push(b'\r');
        bytes
    }
    #[cfg(all(feature = "desktop", target_os = "macos"))]
    fn stored(&self) -> Zeroizing<Vec<u8>> {
        let mut bytes = Zeroizing::new(self.account.as_bytes().to_vec());
        bytes.push(0);
        bytes.extend_from_slice(&self.password);
        bytes
    }
    #[cfg(all(feature = "desktop", target_os = "macos"))]
    fn from_stored(bytes: Zeroizing<Vec<u8>>) -> Result<Self, &'static str> {
        let Some(separator) = bytes.iter().position(|b| *b == 0) else {
            return Err("鑰匙圈資料格式不符；請重新輸入");
        };
        let (account, password) = (&bytes[..separator], &bytes[separator + 1..]);
        let account = std::str::from_utf8(account).map_err(|_| "鑰匙圈帳號格式不符")?;
        Self::new(account.to_owned(), Zeroizing::new(password.to_vec()))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prompt {
    Account,
    Password,
}

/// Only the empty input at PTT's fixed login rows can receive credentials.
/// Text in an article, a partially typed input or an unfinished ANSI sequence cannot.
pub fn prompt(frame: &Frame) -> Option<Prompt> {
    if frame.cols != 80 || frame.rows != 24 || !frame.control_complete {
        return None;
    }
    if crate::navigation::analyze(frame).kind() != "unknown" {
        return None;
    }
    let rows = text::rows(frame)?;
    if ["主功能表", "看板列表", "我的最愛", "分類看板"]
        .iter()
        .any(|title| rows[0].bar_text(title))
        || rows[2].bar_text("作者")
    {
        return None;
    }
    let (col, row) = frame.cursor;
    let kind = match row {
        20 => Prompt::Account,
        21 => Prompt::Password,
        _ => return None,
    };
    let line = rows.get(row)?;
    if !line.mapped() || col == 0 || col >= 80 || !line.cells(col, 80)?.trim().is_empty() {
        return None;
    }
    let prefix = line.cells(0, col)?.trim();
    match kind {
        Prompt::Account if prefix == "請輸入代號，或以 guest 參觀，或以 new 註冊:" => {
            Some(kind)
        }
        Prompt::Password if matches!(prefix, "請輸入您的密碼:" | "請輸入您的密碼：") => {
            Some(kind)
        }
        _ => None,
    }
}

/// The preceding row must echo the exact account from this login attempt.
pub fn password_prompt(frame: &Frame, account: &str) -> bool {
    if prompt(frame) != Some(Prompt::Password) {
        return false;
    }
    let Some(rows) = text::rows(frame) else {
        return false;
    };
    rows[20].mapped()
        && rows[20].text.trim() == format!("請輸入代號，或以 guest 參觀，或以 new 註冊: {account}")
}

#[cfg(all(test, feature = "desktop", target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn stored_secret_round_trip_and_invalid_store_are_explicit() {
        let original = Credentials::new(
            "StellaTest".into(),
            Zeroizing::new(b"synthetic+42".to_vec()),
        )
        .unwrap();
        let restored = Credentials::from_stored(original.stored()).unwrap();
        assert_eq!(restored.account, "StellaTest");
        assert_eq!(*restored.password_input(), b"synthetic+42\r");
        for bytes in [
            b"broken".as_slice(),
            b"guest\0abc",
            b"StellaTest\0abc\0def",
            b"StellaTest\0",
        ] {
            assert!(Credentials::from_stored(Zeroizing::new(bytes.to_vec())).is_err());
        }
    }
}
