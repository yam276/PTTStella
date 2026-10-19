//! Minimal BBS Telnet framing inside the selected WebSocket transport.
#[derive(Default)]
enum State {
    #[default]
    Data,
    Iac,
    Command(u8),
    Sub(Vec<u8>),
    SubIac(Vec<u8>),
    DiscardSub,
    DiscardSubIac,
}
#[derive(Default)]
pub struct Framing {
    state: State,
    local: [bool; 32],
    remote: [bool; 32],
}
impl Framing {
    /// Returns display bytes and protocol replies separately; replies are never drawn.
    pub fn feed(&mut self, input: &[u8], cols: u16, rows: u16) -> (Vec<u8>, Vec<u8>) {
        let mut data = Vec::new();
        let mut reply = Vec::new();
        for &byte in input {
            match std::mem::take(&mut self.state) {
                State::Data => {
                    if byte == 255 {
                        self.state = State::Iac;
                    } else {
                        data.push(byte);
                    }
                }
                State::Iac => match byte {
                    255 => data.push(255),
                    251..=254 => self.state = State::Command(byte),
                    250 => self.state = State::Sub(Vec::new()),
                    _ => {}
                },
                State::Command(command) => self.negotiate(command, byte, cols, rows, &mut reply),
                State::Sub(mut buffer) => {
                    if byte == 255 {
                        self.state = State::SubIac(buffer);
                    } else if buffer.len() < 64 {
                        buffer.push(byte);
                        self.state = State::Sub(buffer);
                    } else {
                        self.state = State::DiscardSub;
                    }
                }
                State::SubIac(mut buffer) => {
                    if byte == 240 {
                        if buffer == [24, 1] {
                            reply.extend_from_slice(b"\xff\xfa\x18\x00ANSI\xff\xf0");
                        }
                    } else if byte == 255 && buffer.len() < 64 {
                        buffer.push(255);
                        self.state = State::Sub(buffer);
                    } else {
                        self.state = State::DiscardSub;
                    }
                }
                State::DiscardSub => {
                    self.state = if byte == 255 {
                        State::DiscardSubIac
                    } else {
                        State::DiscardSub
                    }
                }
                State::DiscardSubIac => {
                    self.state = if byte == 240 {
                        State::Data
                    } else {
                        State::DiscardSub
                    }
                }
            }
        }
        (data, reply)
    }
    fn negotiate(&mut self, cmd: u8, option: u8, cols: u16, rows: u16, reply: &mut Vec<u8>) {
        let index = usize::from(option);
        match cmd {
            253 if matches!(option, 0 | 3 | 24 | 31) => {
                if !self.local[index] {
                    self.local[index] = true;
                    reply.extend([255, 251, option]);
                    if option == 31 {
                        reply.extend([255, 250, 31]);
                        reply.extend(escape(&[cols.to_be_bytes(), rows.to_be_bytes()].concat()));
                        reply.extend([255, 240]);
                    }
                }
            }
            253 => reply.extend([255, 252, option]),
            251 if matches!(option, 0 | 1 | 3) => {
                if !self.remote[index] {
                    self.remote[index] = true;
                    reply.extend([255, 253, option]);
                }
            }
            251 => reply.extend([255, 254, option]),
            254 if index < self.local.len() => self.local[index] = false,
            252 if index < self.remote.len() => self.remote[index] = false,
            _ => {}
        }
    }
}
pub fn escape(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(bytes.len());
    for &byte in bytes {
        output.push(byte);
        if byte == 255 {
            output.push(byte);
        }
    }
    output
}
