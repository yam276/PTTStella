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
            Self::Empty => f.write_str(&crate::msg!("input-empty")),
            Self::TooLong => f.write_str(&crate::msg!("input-bytes-limit")),
            Self::Unencodable => f.write_str(&crate::msg!("input-unencodable")),
            Self::Control(code) => f.write_str(&crate::msg!(
                "input-control",
                value1 = format!("{:04X}", u32::from(*code))
            )),
        }
    }
}
impl std::error::Error for InputError {}

pub fn encode(text: &str) -> Result<Vec<u8>, InputError> {
    encode_with(text, encoding::TextEncoding::Uao)
}
pub fn encode_with(text: &str, selected: encoding::TextEncoding) -> Result<Vec<u8>, InputError> {
    if text.len() > MAX_INPUT_TEXT_BYTES {
        return Err(InputError::TooLong);
    }
    let bytes = selected.encode(text).map_err(|_| InputError::Unencodable)?;
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
    prepare_paste_with(text, encoding::TextEncoding::Uao)
}
pub fn prepare_paste_with(
    text: &str,
    selected: encoding::TextEncoding,
) -> Result<PreparedPaste, InputError> {
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
    let bytes = encode_with(&text.replace('\n', "\r"), selected)?;
    let lines = text.split('\n').count();
    Ok(PreparedPaste { text, bytes, lines })
}
