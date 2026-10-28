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

/// An explicit paste into PTT's editor may insert colour SGR, but no terminal
/// movement, OSC, editor commands or other controls. ESC is typed as Ctrl+U,
/// the editor's insert-ESC command, rather than sent as a terminal escape key.
pub fn prepare_editor_paste(
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
    let mut rest = text.as_str();
    while !rest.is_empty() {
        if let Some(sgr) = rest.strip_prefix("\x1b[") {
            let end = sgr
                .find('m')
                .filter(|end| *end <= 64)
                .ok_or(InputError::Control('\x1b'))?;
            let parameters = &sgr[..end];
            if !parameters.split(';').all(|part| part.is_empty() || part.bytes().all(|b| b.is_ascii_digit()) && part.parse::<u16>().is_ok_and(|n| matches!(n, 0 | 1 | 4 | 5 | 7 | 22 | 24 | 25 | 27 | 30..=37 | 39 | 40..=47 | 49 | 66 | 90..=97 | 100..=107))) {
                return Err(InputError::Control('\x1b'));
            }
            rest = &sgr[end + 1..];
        } else {
            let Some(ch) = rest.chars().next() else { break };
            if ch.is_control() && !matches!(ch, '\n' | '\t') {
                return Err(InputError::Control(ch));
            }
            rest = &rest[ch.len_utf8()..];
        }
    }
    let bytes = encode_with(&text.replace('\x1b', "\x15").replace('\n', "\r"), selected)?;
    let lines = text.split('\n').count();
    Ok(PreparedPaste { text, bytes, lines })
}
