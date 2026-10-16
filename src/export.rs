//! Budget and row geometry for PNGs produced by our browser renderer.
use std::ops::Range;
pub const WIDTH: u32 = 960;
pub const HEIGHT: u32 = 576;
pub const ROWS: usize = 24;
pub const ROW_HEIGHT: u32 = HEIGHT / ROWS as u32;
// A 960x576 RGBA bitmap is 2.11 MiB before PNG scanline/chunk overhead.
pub const MAX_PNG_BYTES: usize = 3 * 1024 * 1024;
pub const MAX_BASE64_BYTES: usize = MAX_PNG_BYTES.div_ceil(3) * 4;

/// This checks bridge shape/budget, not arbitrary PNG image validity.
pub fn check_export_header(bytes: &[u8], rows: &Range<usize>) -> Result<(), &'static str> {
    if rows.start >= rows.end || rows.end > ROWS {
        return Err("PNG 行範圍不正確");
    }
    let height = (rows.end - rows.start) as u32 * ROW_HEIGHT;
    if bytes.len() > MAX_PNG_BYTES {
        return Err("PNG 超過匯出大小上限");
    }
    if bytes.len() < 33
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || bytes[8..12] != 13u32.to_be_bytes()
        || &bytes[12..16] != b"IHDR"
        || bytes[16..20] != WIDTH.to_be_bytes()
        || bytes[20..24] != height.to_be_bytes()
        || bytes[24] != 8
        || !matches!(bytes[25], 2 | 6)
        || bytes[26..29] != [0, 0, 0]
    {
        return Err("PNG 匯出標頭或尺寸不正確");
    }
    Ok(())
}
