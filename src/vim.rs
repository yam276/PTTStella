//! Keyboard policy for browsing. Native text still belongs to the editing host.
use crate::{
    input::MAX_INPUT_BYTES,
    navigation::{Context, Intent, UnknownReason},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct Key {
    pub key: String,
    pub code: String,
    pub ctrl: bool,
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
    pub repeat: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    Native(Vec<u8>),
    Browse(Intent),
    Notice(&'static str),
}
#[derive(Default)]
pub struct Keyboard {
    enabled: bool,
    input: bool,
    once: bool,
    count: usize,
    invalid_count: bool,
    g: bool,
    scroll: Option<usize>,
    profile: &'static str,
    hold: Option<String>,
    text: Option<bool>,
    composition: bool,
}
#[derive(Serialize)]
pub struct Status {
    pub enabled: bool,
    pub mode: &'static str,
    pub pending: bool,
    pub prefix: String,
    pub suspended_reason: Option<UnknownReason>,
}
impl Keyboard {
    pub fn status(&self, context: &Context) -> Status {
        Status {
            enabled: self.enabled,
            suspended_reason: match context {
                Context::Unknown { reason } if self.enabled && !self.input && !self.once => {
                    Some(*reason)
                }
                _ => None,
            },
            mode: if !self.enabled || self.input {
                "input"
            } else if self.once {
                "once"
            } else if matches!(context, Context::Unknown { .. }) {
                "suspended"
            } else {
                "normal"
            },
            pending: self.once
                || self.g
                || self.count > 0
                || self.invalid_count
                || self.hold.is_some()
                || self.composition,
            prefix: format!(
                "{}{}",
                if self.count > 0 {
                    self.count.to_string()
                } else {
                    String::new()
                },
                if self.g { "g" } else { "" }
            ),
        }
    }
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        self.input = false;
        self.reset();
    }
    pub fn arm(&mut self) {
        if self.enabled && !self.input {
            let once = !self.once;
            self.reset();
            self.once = once;
        }
    }
    pub fn reset(&mut self) {
        self.count = 0;
        self.invalid_count = false;
        self.g = false;
        self.once = false;
        self.hold = None;
        self.text = None;
        self.composition = false;
    }
    pub fn context(&mut self, context: &Context) {
        if self.profile != context.kind() {
            self.reset();
            self.scroll = None;
            self.profile = context.kind();
        }
    }
    pub fn release(&mut self, code: &str) {
        if self.hold.as_deref() == Some(code) {
            self.hold = None;
        }
    }
    pub fn composition_start(&mut self) {
        self.count = 0;
        self.g = false;
        self.composition = !self.enabled || self.input || self.once;
    }
    pub fn composition_end(&mut self) {
        self.composition = false;
        if self.once {
            self.once = false;
        }
        self.text = Some(false);
    }
    pub fn accept_text(&mut self, composition: bool) -> bool {
        let allowed = if composition {
            self.composition
        } else {
            self.text.take().unwrap_or(!self.enabled || self.input)
        };
        if allowed && self.once {
            self.once = false;
        }
        allowed
    }
    pub fn independent_edit(&mut self) {
        self.text = None;
    }
    pub fn key(&mut self, key: &Key, context: &Context) -> Effect {
        self.context(context);
        // Normal mode is a command surface even with a CJK input source active.
        // WebKit can report Process/229 instead of a Latin key. Use the physical
        // code only in browsing modes; native text/composition keeps its owner.
        let normalized = if self.enabled && !self.input && !self.once {
            normal_key(key)
        } else {
            None
        };
        let key = normalized.as_ref().unwrap_or(key);
        // Option/Alt may produce an ordinary text edit on some layouts. Let the
        // editing host provide it in native mode instead of discarding the edit.
        let printable = key.key.chars().count() == 1 && !key.ctrl;
        if matches!(
            key.key.as_str(),
            "Shift" | "Control" | "Alt" | "Meta" | "CapsLock"
        ) {
            return Effect::None;
        }
        self.text = Some(false);
        if self.hold.as_deref() == Some(&key.code) {
            return Effect::None;
        }
        if !self.enabled || self.input || self.once {
            if self.enabled
                && self.input
                && !key.repeat
                && (key.key == "Escape" || key.ctrl && key.key == "[")
                && !matches!(context, Context::Unknown { .. })
            {
                self.input = false;
                self.reset();
                return Effect::None;
            }
            if self.once {
                self.hold = Some(key.code.clone());
            }
            if printable {
                self.text = Some(true);
                return Effect::None;
            }
            let bytes = native_key(key);
            if self.once && bytes.is_some() {
                self.once = false;
            }
            return bytes.map_or(Effect::None, Effect::Native);
        }
        if key.key == "Escape" || key.ctrl && key.key == "[" {
            self.reset();
            return Effect::None;
        }
        if key.key == "f" && !key.ctrl && !key.alt && !key.repeat {
            if self.count > 0 || self.g {
                self.reset();
                return Effect::Notice("原生一次不接受計數或前綴");
            }
            self.once = true;
            self.hold = Some(key.code.clone());
            return Effect::None;
        }
        if key.key == "i" && !key.ctrl && !key.alt && !key.repeat {
            self.input = true;
            self.reset();
            self.text = Some(false);
            return Effect::None;
        }
        if let Context::Unknown { reason } = context {
            self.reset();
            return Effect::Notice(match reason {
                UnknownReason::UnsupportedGrid => "Vim 已暫停：不支援的格線；按 i 原始輸入",
                UnknownReason::Incomplete => "Vim 已暫停：畫面接收未完成；按 i 原始輸入",
                UnknownReason::Unmapped => "Vim 已暫停：操作區有未映射字格；按 i 原始輸入",
                UnknownReason::Unrecognized => "Vim 已暫停：未辨識的畫面格式；按 i 原始輸入",
                UnknownReason::BoardHeader => "Vim 已暫停：看板標頭未確認；按 i 原始輸入",
                UnknownReason::BoardRows => "Vim 已暫停：看板項目欄位未確認；按 i 原始輸入",
                UnknownReason::InvalidCursor => {
                    "Vim 已暫停：選取指標或游標位置未確認；按 i 原始輸入"
                }
            });
        }
        if !key.ctrl && !key.alt && key.key.len() == 1 && key.key.as_bytes()[0].is_ascii_digit() {
            if self.invalid_count {
                return Effect::None;
            }
            let digit = (key.key.as_bytes()[0] - b'0') as usize;
            if self.g {
                self.reset();
                return Effect::Notice("不支援此計數指令");
            }
            if let Some(count) = self
                .count
                .checked_mul(10)
                .and_then(|n| n.checked_add(digit))
                .filter(|n| *n <= MAX_INPUT_BYTES / 3)
            {
                self.count = count;
                return Effect::None;
            }
            self.reset();
            self.invalid_count = true;
            return Effect::Notice("導覽計數超過輸入容量；整個指令未送出");
        }
        if self.invalid_count {
            self.reset();
            return Effect::Notice("超限計數的整個指令未送出");
        }
        let count = self.count.max(1);
        let counted = self.count > 0;
        self.count = 0;
        if self.g {
            self.g = false;
            return if key.key == "g" && !key.ctrl && !counted {
                Effect::Browse(Intent::First)
            } else {
                Effect::Notice("不支援此 g 指令")
            };
        }
        if key.alt {
            return Effect::Notice("此組合未提供 Vim 瀏覽");
        }
        let action = if key.ctrl {
            match key.key.to_ascii_lowercase().as_str() {
                "d" | "u" => {
                    let rows = context.content_rows();
                    if counted {
                        self.scroll = Some(count);
                    }
                    Intent::Lines {
                        down: key.key.eq_ignore_ascii_case("d"),
                        count: self.scroll.unwrap_or((rows / 2).max(1)),
                    }
                }
                "e" | "y" => Intent::Lines {
                    down: key.key.eq_ignore_ascii_case("e"),
                    count,
                },
                "f" if !counted => Intent::NextPage,
                "b" if !counted => Intent::PreviousPage,
                _ => return Effect::Notice("此組合或計數未提供 Vim 瀏覽"),
            }
        } else {
            match key.key.as_str() {
                "j" | "ArrowDown" => Intent::Lines { down: true, count },
                "k" | "ArrowUp" => Intent::Lines { down: false, count },
                "h" | "ArrowLeft" if !counted => Intent::Back,
                "l" | "ArrowRight" | "Enter" if !counted => Intent::Forward,
                "g" if !counted && !key.repeat => {
                    self.g = true;
                    return Effect::None;
                }
                "G" | "End" if !counted => Intent::Last,
                "Home" if !counted => Intent::First,
                "PageDown" if !counted => Intent::NextPage,
                "PageUp" if !counted => Intent::PreviousPage,
                _ => {
                    return Effect::Notice(
                        "此按鍵或計數未提供 Vim 瀏覽；按 f 原生一次，或 i 原始輸入",
                    );
                }
            }
        };
        Effect::Browse(action)
    }
}
fn normal_key(key: &Key) -> Option<Key> {
    if key.key != "Process" && key.key.is_ascii() {
        return None;
    }
    let letter = key
        .code
        .strip_prefix("Key")
        .filter(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase());
    let digit = key
        .code
        .strip_prefix("Digit")
        .filter(|s| !key.shift && s.len() == 1 && s.as_bytes()[0].is_ascii_digit());
    let value = letter.or(digit)?;
    let mut normalized = key.clone();
    normalized.key = if key.shift {
        value.to_ascii_uppercase()
    } else {
        value.to_ascii_lowercase()
    };
    Some(normalized)
}
fn native_key(key: &Key) -> Option<Vec<u8>> {
    if key.alt {
        return None;
    }
    if key.ctrl && key.key.len() == 1 && key.key.as_bytes()[0].is_ascii_alphabetic() {
        return Some(vec![key.key.as_bytes()[0].to_ascii_uppercase() & 31]);
    }
    Some(
        match key.key.as_str() {
            "Enter" => b"\r".as_slice(),
            "Backspace" => b"\x08",
            "Tab" => b"\t",
            "Escape" => b"\x1b",
            "ArrowUp" => b"\x1b[A",
            "ArrowDown" => b"\x1b[B",
            "ArrowRight" => b"\x1b[C",
            "ArrowLeft" => b"\x1b[D",
            "Home" => b"\x1b[H",
            "End" => b"\x1b[F",
            "PageUp" => b"\x1b[5~",
            "PageDown" => b"\x1b[6~",
            "Delete" => b"\x1b[3~",
            _ => return None,
        }
        .to_vec(),
    )
}
