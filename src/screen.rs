use crate::encoding;
use serde::{Deserialize, Serialize};

const MAX_CELLS: usize = 80 * 100;
const MAX_CSI: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Style {
    pub fg: u8,
    pub bg: u8,
    pub bright: bool,
    pub inverse: bool,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            fg: 7,
            bg: 0,
            bright: false,
            inverse: false,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub byte: u8,
    pub style: Style,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Glyph {
    pub col: usize,
    pub row: usize,
    pub width: usize,
    pub text: String,
    pub left: Style,
    pub right: Style,
    pub unmapped: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Frame {
    pub control_complete: bool,
    pub cols: usize,
    pub rows: usize,
    pub cursor: (usize, usize),
    pub cursor_visible: bool,
    pub revision: u64,
    pub glyphs: Vec<Glyph>,
    pub unsupported: u64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Parser {
    #[default]
    Ground,
    Escape,
    Csi(Vec<u8>),
    String,
    StringEscape,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    cols: usize,
    rows: usize,
    cells: Vec<Cell>,
    x: usize,
    y: usize,
    saved: (usize, usize),
    style: Style,
    parser: Parser,
    top: usize,
    bottom: usize,
    wrap: bool,
    auto_wrap: bool,
    cursor_visible: bool,
    revision: u64,
    unsupported: u64,
}
impl Screen {
    pub fn new(cols: usize, rows: usize) -> Result<Self, &'static str> {
        if cols == 0 || rows == 0 || cols.checked_mul(rows).is_none_or(|n| n > MAX_CELLS) {
            return Err("unsupported screen dimensions");
        }
        Ok(Self {
            cols,
            rows,
            cells: vec![Cell::default(); cols * rows],
            x: 0,
            y: 0,
            saved: (0, 0),
            style: Style::default(),
            parser: Parser::Ground,
            top: 0,
            bottom: rows - 1,
            wrap: false,
            auto_wrap: true,
            cursor_visible: true,
            revision: 0,
            unsupported: 0,
        })
    }
    pub fn cell(&self, x: usize, y: usize) -> Option<Cell> {
        (x < self.cols && y < self.rows).then(|| self.cells[y * self.cols + x])
    }
    pub fn cursor(&self) -> (usize, usize) {
        (self.x, self.y)
    }
    pub fn feed(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.byte(byte);
        }
    }
    /// A new connection cannot inherit a partially received control sequence.
    pub fn reset_session(&mut self) {
        self.reset();
        self.parser = Parser::Ground;
        self.revision = 0;
        self.unsupported = 0;
    }
    fn byte(&mut self, byte: u8) {
        let parser = std::mem::take(&mut self.parser);
        match parser {
            Parser::Ground => self.ground(byte),
            Parser::Escape => match byte {
                b'[' => self.parser = Parser::Csi(Vec::new()),
                b']' | b'P' | b'^' | b'_' => self.parser = Parser::String,
                b'7' => self.saved = (self.x, self.y),
                b'8' => {
                    (self.x, self.y) = self.saved;
                    self.wrap = false;
                }
                b'D' => self.linefeed(),
                b'M' => {
                    if self.y == self.top {
                        self.scroll_down(1);
                    } else {
                        self.y = self.y.saturating_sub(1);
                    }
                }
                b'E' => {
                    self.x = 0;
                    self.linefeed();
                }
                b'c' => self.reset(),
                0x1b => self.parser = Parser::Escape,
                _ => self.unsupported += 1,
            },
            Parser::Csi(mut data) => {
                if byte == 0x1b {
                    self.parser = Parser::Escape;
                } else if (0x40..=0x7e).contains(&byte) {
                    self.csi(&data, byte);
                } else if data.len() < MAX_CSI && (0x20..=0x3f).contains(&byte) {
                    data.push(byte);
                    self.parser = Parser::Csi(data);
                } else {
                    self.unsupported += 1;
                }
            }
            Parser::String => {
                self.parser = match byte {
                    7 => Parser::Ground,
                    0x1b => Parser::StringEscape,
                    _ => Parser::String,
                }
            }
            Parser::StringEscape => {
                self.parser = if byte == b'\\' {
                    Parser::Ground
                } else {
                    Parser::String
                }
            }
        }
        self.revision = self.revision.saturating_add(1);
    }
    fn reset(&mut self) {
        self.cells.fill(Cell::default());
        self.x = 0;
        self.y = 0;
        self.saved = (0, 0);
        self.top = 0;
        self.bottom = self.rows - 1;
        self.style = Style::default();
        self.wrap = false;
        self.auto_wrap = true;
        self.cursor_visible = true;
    }
    fn ground(&mut self, byte: u8) {
        match byte {
            0x1b => self.parser = Parser::Escape,
            b'\r' => {
                self.x = 0;
                self.wrap = false;
            }
            b'\n' | 0x0b | 0x0c => self.linefeed(),
            8 => {
                self.x = self.x.saturating_sub(1);
                self.wrap = false;
            }
            9 => {
                self.x = ((self.x / 8 + 1) * 8).min(self.cols - 1);
                self.wrap = false;
            }
            0..=0x1f | 0x7f => {}
            _ => {
                if self.wrap && self.auto_wrap {
                    self.x = 0;
                    self.linefeed();
                }
                self.cells[self.y * self.cols + self.x] = Cell {
                    byte,
                    style: self.style,
                };
                self.wrap = self.x == self.cols - 1;
                if !self.wrap {
                    self.x += 1;
                }
            }
        }
    }
    fn blank(&self) -> Cell {
        Cell {
            byte: 0,
            style: self.style,
        }
    }
    fn linefeed(&mut self) {
        self.wrap = false;
        if self.y == self.bottom {
            self.scroll_up(1);
        } else {
            self.y = (self.y + 1).min(self.rows - 1);
        }
    }
    fn scroll_up(&mut self, count: usize) {
        let n = count.min(self.bottom - self.top + 1);
        let start = self.top * self.cols;
        let end = (self.bottom + 1) * self.cols;
        self.cells.copy_within(start + n * self.cols..end, start);
        let blank = self.blank();
        self.cells[end - n * self.cols..end].fill(blank);
    }
    fn scroll_down(&mut self, count: usize) {
        let n = count.min(self.bottom - self.top + 1);
        let start = self.top * self.cols;
        let end = (self.bottom + 1) * self.cols;
        self.cells
            .copy_within(start..end - n * self.cols, start + n * self.cols);
        let blank = self.blank();
        self.cells[start..start + n * self.cols].fill(blank);
    }
    fn csi(&mut self, bytes: &[u8], final_byte: u8) {
        let private = bytes.first() == Some(&b'?');
        let bytes = if private { &bytes[1..] } else { bytes };
        if !bytes.iter().all(|b| b.is_ascii_digit() || *b == b';') {
            self.unsupported += 1;
            return;
        }
        let mut p = Vec::new();
        for part in bytes.split(|b| *b == b';') {
            let n = part
                .iter()
                .fold(0usize, |n, b| (n * 10 + usize::from(b - b'0')).min(9999));
            p.push(n);
        }
        let first = p.first().copied().unwrap_or(0);
        let n = first.max(1);
        if private {
            match (first, final_byte) {
                (25, b'h') => self.cursor_visible = true,
                (25, b'l') => self.cursor_visible = false,
                (7, b'h') => self.auto_wrap = true,
                (7, b'l') => self.auto_wrap = false,
                _ => self.unsupported += 1,
            }
            return;
        }
        match final_byte {
            b'm' => self.sgr(&p),
            b'H' | b'f' => {
                self.y = (n - 1).min(self.rows - 1);
                self.x = (p.get(1).copied().unwrap_or(1).max(1) - 1).min(self.cols - 1);
            }
            b'A' => self.y = self.y.saturating_sub(n),
            b'B' => self.y = (self.y + n).min(self.rows - 1),
            b'C' => self.x = (self.x + n).min(self.cols - 1),
            b'D' => self.x = self.x.saturating_sub(n),
            b'G' | b'`' => self.x = (n - 1).min(self.cols - 1),
            b'd' => self.y = (n - 1).min(self.rows - 1),
            b'J' => {
                let at = self.y * self.cols + self.x;
                let blank = self.blank();
                match first {
                    0 => self.cells[at..].fill(blank),
                    1 => self.cells[..=at].fill(blank),
                    2 | 3 => self.cells.fill(blank),
                    _ => self.unsupported += 1,
                }
            }
            b'K' => {
                let start = self.y * self.cols;
                let blank = self.blank();
                match first {
                    0 => self.cells[start + self.x..start + self.cols].fill(blank),
                    1 => self.cells[start..=start + self.x].fill(blank),
                    2 => self.cells[start..start + self.cols].fill(blank),
                    _ => self.unsupported += 1,
                }
            }
            b'r' => {
                let top = n - 1;
                let bottom = p.get(1).copied().filter(|n| *n != 0).unwrap_or(self.rows) - 1;
                if top < bottom && bottom < self.rows {
                    self.top = top;
                    self.bottom = bottom;
                    self.x = 0;
                    self.y = 0;
                } else {
                    self.unsupported += 1;
                }
            }
            b'S' => self.scroll_up(n),
            b'T' => self.scroll_down(n),
            b's' => self.saved = (self.x, self.y),
            b'u' => (self.x, self.y) = self.saved,
            b'P' | b'@' | b'X' => self.edit_chars(n, final_byte),
            b'L' | b'M' => {
                if self.y >= self.top && self.y <= self.bottom {
                    let old_top = self.top;
                    self.top = self.y;
                    if final_byte == b'L' {
                        self.scroll_down(n);
                    } else {
                        self.scroll_up(n);
                    }
                    self.top = old_top;
                }
            }
            _ => self.unsupported += 1,
        }
        // SGR must not resolve pending wrap: two differently coloured byte cells
        // can occupy the final two columns of the same glyph.
        if final_byte != b'm' {
            self.wrap = false;
        }
    }
    fn edit_chars(&mut self, n: usize, op: u8) {
        let n = n.min(self.cols - self.x);
        let start = self.y * self.cols + self.x;
        let end = (self.y + 1) * self.cols;
        let blank = self.blank();
        match op {
            b'P' => {
                self.cells.copy_within(start + n..end, start);
                self.cells[end - n..end].fill(blank);
            }
            b'@' => {
                self.cells.copy_within(start..end - n, start + n);
                self.cells[start..start + n].fill(blank);
            }
            _ => self.cells[start..start + n].fill(blank),
        }
    }
    fn sgr(&mut self, params: &[usize]) {
        if params.iter().any(|p| matches!(p, 38 | 48)) {
            self.unsupported += 1;
            return;
        }
        for &p in params {
            match p {
                0 => self.style = Style::default(),
                1 => self.style.bright = true,
                22 => self.style.bright = false,
                7 => self.style.inverse = true,
                27 => self.style.inverse = false,
                30..=37 => self.style.fg = (p - 30) as u8,
                40..=47 => self.style.bg = (p - 40) as u8,
                90..=97 => self.style.fg = (p - 90 + 8) as u8,
                100..=107 => self.style.bg = (p - 100 + 8) as u8,
                39 => self.style.fg = 7,
                49 => self.style.bg = 0,
                _ => self.unsupported += 1,
            }
        }
    }
    pub fn frame(&self) -> Frame {
        let mut glyphs = Vec::with_capacity(self.cells.len());
        for row in 0..self.rows {
            let mut col = 0;
            while col < self.cols {
                let left = self.cells[row * self.cols + col];
                let pair = if left.byte >= 0x81 && col + 1 < self.cols {
                    encoding::pair([left.byte, self.cells[row * self.cols + col + 1].byte])
                } else {
                    None
                };
                let width = if pair.is_some() { 2 } else { 1 };
                let unmapped = pair.is_none() && left.byte >= 0x80;
                let text = pair.unwrap_or_else(|| match left.byte {
                    0 => " ".into(),
                    0x20..=0x7e => char::from(left.byte).to_string(),
                    _ => "\u{fffd}".into(),
                });
                glyphs.push(Glyph {
                    col,
                    row,
                    width,
                    text,
                    left: left.style,
                    right: self.cells[row * self.cols + col + width - 1].style,
                    unmapped,
                });
                col += width;
            }
        }
        Frame {
            control_complete: matches!(self.parser, Parser::Ground),
            cols: self.cols,
            rows: self.rows,
            cursor: (self.x, self.y),
            cursor_visible: self.cursor_visible,
            revision: self.revision,
            glyphs,
            unsupported: self.unsupported,
        }
    }
}
