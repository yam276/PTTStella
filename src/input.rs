//! Input size and paste preparation share one encoded-byte boundary.
use crate::encoding;
use std::fmt;

pub const MAX_INPUT_BYTES: usize = 4096;
pub const MAX_INPUT_TEXT_BYTES: usize = MAX_INPUT_BYTES * 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputError {
    Empty,
    TooLong,
    Unencodable,
    Control(char),
}
impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("剪貼簿沒有文字"),
            Self::TooLong => f.write_str("輸入超過 4096 個 Big5-UAO bytes；整段未送出"),
            Self::Unencodable => f.write_str("文字含無法以 Big5-UAO 傳送的字元；整段未送出"),
            Self::Control(ch) => write!(
                f,
                "一般貼上不接受控制字元 U+{:04X}；整段未送出",
                u32::from(*ch)
            ),
        }
    }
}
impl std::error::Error for InputError {}

pub fn encode(text: &str) -> Result<Vec<u8>, InputError> {
    if text.len() > MAX_INPUT_TEXT_BYTES {
        return Err(InputError::TooLong);
    }
    let bytes = encoding::encode(text).map_err(|_| InputError::Unencodable)?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(InputError::TooLong);
    }
    Ok(bytes)
}

#[derive(Clone, Debug)]
pub struct PreparedPaste {
    pub text: String,
    pub bytes: Vec<u8>,
    pub lines: usize,
}

pub fn prepare_paste(text: &str) -> Result<PreparedPaste, InputError> {
    if text.is_empty() {
        return Err(InputError::Empty);
    }
    if text.len() > MAX_INPUT_TEXT_BYTES {
        return Err(InputError::TooLong);
    }
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    if let Some(ch) = text
        .chars()
        .find(|ch| ch.is_control() && !matches!(ch, '\n' | '\t'))
    {
        return Err(InputError::Control(ch));
    }
    let bytes = encode(&text.replace('\n', "\r"))?;
    let lines = text.split('\n').count();
    Ok(PreparedPaste { text, bytes, lines })
}
