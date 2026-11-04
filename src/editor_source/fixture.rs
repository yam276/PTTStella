//! Independent synthetic station. The save menu rejects every choice except T/E.
use crate::{
    editor_color::fixture::Editor,
    encoding,
    screen::{Frame, Screen},
};
pub struct Native {
    pub editor: Editor,
    pub title: String,
    mode: u8,
    revision: u64,
    pub menu_count: usize,
}
impl Native {
    pub fn new(lines: &[&str], title: &str, preview: bool) -> Self {
        let mut editor = Editor::new(lines, preview, true, true);
        editor.eof_markers = true;
        Self {
            editor,
            title: title.into(),
            mode: 0,
            revision: 0,
            menu_count: 0,
        }
    }
    pub fn wire(&self) -> Vec<u8> {
        match self.mode {
            1=>encoding::encode("\x1b[2J\x1b[H檔案處理\x1b[2;1H[S]發文 (T)改標題 (A)放棄 (E)繼續 (R/W/D)讀寫刪暫存檔\x1b[3;1H確定要儲存檔案嗎？ ").unwrap(),
            2=>encoding::encode(&format!("\x1b[2J\x1b[H檔案處理\x1b[4;1H舊標題：{}\x1b[5;1H新標題：{}",self.title,self.title)).unwrap(),
            _=>self.editor.wire()
        }
    }
    pub fn frame(&mut self) -> Frame {
        let mut s = Screen::new(80, 24).unwrap();
        s.feed(&self.wire());
        let mut f = s.frame();
        self.revision += 1;
        f.revision = self.revision;
        f
    }
    pub fn accept(&mut self, b: &[u8]) {
        match self.mode {
            1 => {
                assert_eq!(b, b"t\r", "must never publish");
                self.mode = 2;
            }
            2 => {
                let n = b
                    .iter()
                    .position(|c| *c == 13)
                    .expect("title must be explicitly completed");
                if n > 0 {
                    assert_eq!(b[0], 25, "clear prefilled title using vget Ctrl+Y");
                    self.title = decode(&b[1..n]);
                }
                self.mode = 0;
                if n + 1 < b.len() {
                    self.editor.accept(&b[n + 1..]);
                }
            }
            _ => {
                if b == [24] {
                    self.mode = 1;
                    self.menu_count += 1;
                } else {
                    assert!(!b.contains(&24));
                    self.editor.accept(b);
                }
            }
        }
    }
}
fn decode(bytes: &[u8]) -> String {
    let mut s = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] >= 128 {
            s.push_str(&encoding::pair([bytes[i], bytes[i + 1]]).unwrap());
            i += 2;
        } else {
            s.push(bytes[i] as char);
            i += 1;
        }
    }
    s
}
