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
