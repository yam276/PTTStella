use crate::{
    encoding,
    screen::{Frame, Screen},
};
// Independent, byte-addressed synthetic editor. It preserves document bytes and
// implements only the standard keys allowed by the production colouring flow.
pub struct Editor {
    pub lines: Vec<Vec<u8>>,
    line: usize,
    pos: usize,
    pub preview: bool,
    pub insert: bool,
    revision: u64,
    pub commands: Vec<Vec<u8>>,
    dbcs: bool,
}
impl Editor {
    pub fn new(lines: &[&str], preview: bool, insert: bool, dbcs: bool) -> Self {
        Self {
            lines: lines.iter().map(|s| encoding::encode(s).unwrap()).collect(),
            line: 0,
            pos: 0,
            preview,
            insert,
            revision: 1,
            commands: Vec::new(),
            dbcs,
        }
    }
    pub fn frame(&mut self) -> Frame {
        self.revision += 1;
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&self.wire());
        let mut frame = screen.frame();
        frame.revision = self.revision;
        frame
    }
    pub fn wire(&self) -> Vec<u8> {
        let mut wire = Vec::new();
        wire.extend_from_slice(b"\x1b[2J");
        for (index, line) in self.lines.iter().take(23).enumerate() {
            wire.extend_from_slice(format!("\x1b[{};1H\x1b[0m", index + 1).as_bytes());
            if self.preview {
                wire.extend_from_slice(line);
            } else {
                wire.extend_from_slice(
                    &line
                        .iter()
                        .map(|b| if *b == 0x1b { b'*' } else { *b })
                        .take(79)
                        .collect::<Vec<_>>(),
                );
            }
        }
        let col = if self.preview {
            visible(&self.lines[self.line][..self.pos])
        } else {
            self.pos
        };
        wire.extend_from_slice(&encoding::encode(&format!("\x1b[24;1H\x1b[30;47m編輯文章 (^X)存檔 (^C)色碼 (^V)彩色║{}│{}ipr║{}:{}\x1b[0m\x1b[{};{}H",if self.insert{"插入"}else{"取代"},if self.preview{"A"}else{"a"},self.line+1,col+1,self.line+1,col.min(79)+1)).unwrap());
        wire
    }
    pub fn accept(&mut self, bytes: &[u8]) {
        self.commands.push(bytes.to_vec());
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                0x16 => self.preview = !self.preview,
                0x0f => self.insert = !self.insert,
                1 => self.pos = 0,
                5 => self.pos = self.lines[self.line].len(),
                0x15 => {
                    self.lines[self.line].insert(self.pos, 0x1b);
                    self.pos += 1;
                }
                0x1b => {
                    if bytes.get(i + 1) == Some(&b'u') {
                        i += 2;
                        continue;
                    }
                    assert_eq!(bytes.get(i + 1), Some(&b'['));
                    match bytes[i + 2] {
                        b'A' => {
                            self.line = self.line.saturating_sub(1);
                            self.pos = self.pos.min(self.lines[self.line].len());
                        }
                        b'B' => {
                            self.line = (self.line + 1).min(self.lines.len() - 1);
                            self.pos = self.pos.min(self.lines[self.line].len());
                        }
                        b'C' | b'D' => {
                            let mut boundaries = vec![0];
                            let mut k = 0;
                            while k < self.lines[self.line].len() {
                                k += if self.lines[self.line][k] >= 0x80 {
                                    2
                                } else {
                                    1
                                };
                                boundaries.push(k);
                            }
                            if bytes[i + 2] == b'C' {
                                self.pos = (self.pos + 1).min(self.lines[self.line].len());
                                if self.dbcs {
                                    self.pos =
                                        *boundaries.iter().find(|p| **p >= self.pos).unwrap();
                                }
                            } else {
                                self.pos = self.pos.saturating_sub(1);
                                if self.dbcs {
                                    self.pos =
                                        *boundaries.iter().rev().find(|p| **p <= self.pos).unwrap();
                                }
                            }
                        }
                        _ => panic!("unexpected cursor command"),
                    }
                    i += 2;
                }
                c if c.is_ascii_graphic() || c == b' ' => {
                    assert!(self.insert, "must explicitly enable insert mode");
                    self.lines[self.line].insert(self.pos, c);
                    self.pos += 1;
                }
                c => panic!("unexpected control {c}"),
            }
            i += 1;
        }
    }
}
fn visible(bytes: &[u8]) -> usize {
    let mut total = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            while i < bytes.len() && bytes[i] != b'm' {
                i += 1;
            }
            i += 1;
        } else {
            total += 1;
            i += 1;
        }
    }
    total
}
