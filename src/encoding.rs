//! Public MIT-licensed uao-rs mapping; no original-client tables are used.
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unencodable;
impl fmt::Display for Unencodable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&crate::msg!("encoding-unencodable"))
    }
}
impl std::error::Error for Unencodable {}

pub fn pair(bytes: [u8; 2]) -> Option<String> {
    let text = uao_rs::decode(&bytes);
    (!text.is_empty() && !text.contains('\u{fffd}')).then_some(text)
}

/// Upstream encode can silently omit unknown characters. Round-trip every scalar,
/// then reject the complete input rather than sending a partially encoded string.
pub fn encode(text: &str) -> Result<Vec<u8>, Unencodable> {
    let mut output = Vec::new();
    for ch in text.chars() {
        if ch.is_ascii() {
            output.push(ch as u8);
        } else {
            let encoded = uao_rs::encode(&ch.to_string());
            if encoded.len() != 2
                || pair([encoded[0], encoded[1]]).as_deref() != Some(&ch.to_string())
            {
                return Err(Unencodable);
            }
            output.extend(encoded);
        }
    }
    Ok(output)
}

/// The byte-per-cell legacy encodings supported by the BBS screen owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextEncoding {
    #[default]
    Uao,
    Big5,
    Gbk,
}
impl TextEncoding {
    pub fn pair(self, bytes: [u8; 2]) -> Option<String> {
        match self {
            Self::Uao => pair(bytes),
            Self::Big5 | Self::Gbk => {
                let decoder = if self == Self::Big5 {
                    encoding_rs::BIG5
                } else {
                    encoding_rs::GBK
                };
                decoder
                    .decode_without_bom_handling_and_without_replacement(&bytes)
                    .filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
                    .map(|text| text.into_owned())
            }
        }
    }
    pub fn single(self, byte: u8) -> Option<char> {
        match byte {
            0 => Some(' '),
            0x20..=0x7e => Some(char::from(byte)),
            0x80 if self == Self::Gbk => Some('€'),
            _ => None,
        }
    }
    pub fn encode(self, text: &str) -> Result<Vec<u8>, Unencodable> {
        if self == Self::Uao {
            return encode(text);
        }
        let codec = if self == Self::Big5 {
            encoding_rs::BIG5
        } else {
            encoding_rs::GBK
        };
        let (bytes, _, failed) = codec.encode(text);
        if failed
            || codec
                .decode_without_bom_handling_and_without_replacement(&bytes)
                .as_deref()
                != Some(text)
        {
            return Err(Unencodable);
        }
        Ok(bytes.into_owned())
    }
}
