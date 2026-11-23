use std::collections::{HashMap, HashSet};
use std::env;
use std::io::{self, Read};
use std::os::fd::{AsFd, AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use wait_timeout::ChildExt;

pub enum InputEvent {
    Move(f64, f64),
    Button(&'static str, bool),
    Key(String, Vec<String>, i32),
    Wheel(i32, i32),
}

const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;
const REL_HWHEEL: u16 = 0x06;
const REL_WHEEL: u16 = 0x08;
const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;
const BTN_SIDE: u16 = 0x113;
const BTN_EXTRA: u16 = 0x114;
const KEY_A: u16 = 30;
const KEY_ENTER: u16 = 28;

pub struct InputManager {
    devices: Vec<evdev::Device>,
    hypr: Option<UnixStream>,
    buffer: Vec<u8>,
    last_poll: Instant,
    last_press: HashMap<u16, Instant>,
    modifiers: HashSet<String>,
}

impl InputManager {
    pub fn new() -> Self {
        let mut enumerated: Vec<_> = evdev::enumerate()
            .filter_map(|(path, device)| {
                let keys = device.supported_keys();
                let rels = device.supported_relative_axes();
                let has_button = keys.is_some_and(|set| [BTN_LEFT, BTN_RIGHT, BTN_MIDDLE, BTN_SIDE, BTN_EXTRA]
                    .iter().any(|code| set.contains(evdev::KeyCode(*code))));
                let has_rel = rels.is_some_and(|set| set.contains(evdev::RelativeAxisCode(REL_X)) || set.contains(evdev::RelativeAxisCode(REL_Y)));
                let is_mouse = has_button && has_rel;
                let is_keyboard = keys.is_some_and(|set| set.contains(evdev::KeyCode(KEY_A)) && set.contains(evdev::KeyCode(KEY_ENTER)));
                if is_mouse || is_keyboard {
                    let _ = device.set_nonblocking(true);
                    Some((path, device))
                } else {
                    None
                }
            })
            .collect();
        enumerated.sort_by(|a, b| a.0.cmp(&b.0));
        let devices: Vec<evdev::Device> = enumerated.into_iter().map(|(_, device)| device).collect();
        if devices.is_empty() {
            println!("[tinyhand] warning: no /dev/input devices available");
        } else {
            println!("[tinyhand] input: {}", devices.iter().take(8).filter_map(|d| d.name().map(|n| n.to_string())).collect::<Vec<_>>().join(", "));
        }

        let mut hypr = None;
        if let Ok(signature) = env::var("HYPRLAND_INSTANCE_SIGNATURE") {
            let runtime = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
            let path = PathBuf::from(runtime).join("hypr").join(signature).join(".socket2.sock");
            if path.exists() {
                if let Ok(stream) = UnixStream::connect(&path) {
                    let _ = stream.set_nonblocking(true);
                    hypr = Some(stream);
                }
            }
        }
        Self {
            devices,
            hypr,
            buffer: Vec::new(),
            last_poll: Instant::now() - Duration::from_secs(1),
            last_press: HashMap::new(),
            modifiers: HashSet::new(),
        }
    }

    pub fn fds(&mut self) -> Vec<RawFd> {
        let mut result: Vec<RawFd> = self.devices.iter().map(|d| d.as_fd().as_raw_fd()).collect();
        if let Some(stream) = &mut self.hypr {
            result.push(stream.as_raw_fd());
        }
        result
    }

    pub fn poll_hypr(&mut self, out: &mut Vec<InputEvent>) {
        if self.last_poll.elapsed() < Duration::from_millis(50) {
            return;
        }
        self.last_poll = Instant::now();
        let mut child = match Command::new("hyprctl")
            .args(["-j", "cursorpos"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => return,
        };
        let status = match child.wait_timeout(Duration::from_millis(500)) {
            Ok(Some(status)) => status,
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
            Err(_) => return,
        };
        if !status.success() {
            return;
        }
        let mut stdout = String::new();
        if let Some(mut pipe) = child.stdout.take() {
            let _ = pipe.read_to_string(&mut stdout);
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&stdout) {
            if let (Some(x), Some(y)) = (value.get("x").and_then(|v| v.as_f64()), value.get("y").and_then(|v| v.as_f64())) {
                out.push(InputEvent::Move(x, y));
            }
        }
    }

    pub fn handle_ready(&mut self, fd: RawFd, out: &mut Vec<InputEvent>) {
        if let Some(stream) = &mut self.hypr {
            if stream.as_raw_fd() == fd {
                let mut chunk = [0u8; 65536];
                match stream.read(&mut chunk) {
                    Ok(0) => return,
                    Ok(n) => self.buffer.extend_from_slice(&chunk[..n]),
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return,
                    Err(_) => return,
                }
                while let Some(pos) = self.buffer.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = self.buffer.drain(..=pos).collect();
                    let text = String::from_utf8_lossy(&line[..line.len() - 1]);
                    if let Some(coords) = text.strip_prefix("pointermove>>") {
                        let mut parts = coords.split(',');
                        if let (Some(x), Some(y)) = (parts.next().and_then(|s| s.parse::<f64>().ok()), parts.next().and_then(|s| s.parse::<f64>().ok())) {
                            out.push(InputEvent::Move(x, y));
                        }
                    }
                }
                return;
            }
        }
        let index = self.devices.iter().position(|d| d.as_fd().as_raw_fd() == fd);
        let Some(index) = index else { return; };
        let events: Vec<(u16, u16, i32)> = match self.devices[index].fetch_events() {
            Ok(events) => events.map(|e| (e.event_type().0, e.code(), e.value())).collect(),
            Err(_) => return,
        };
        for (kind, code, value) in events {
            self.translate(kind, code, value, out);
        }
    }

    fn translate(&mut self, kind: u16, code: u16, value: i32, out: &mut Vec<InputEvent>) {
        if kind == EV_REL {
            if value == 0 { return; }
            if code == REL_WHEEL {
                out.push(InputEvent::Wheel(0, -value as i32));
            } else if code == REL_HWHEEL {
                out.push(InputEvent::Wheel(value as i32, 0));
            }
            return;
        }
        if kind != EV_KEY { return; }

        if let Some(name) = modifier_name(code) {
            if value != 0 {
                self.modifiers.insert(name.to_string());
            } else {
                self.modifiers.remove(name);
            }
            out.push(InputEvent::Key(name.to_string(), self.modifiers.iter().cloned().collect(), value));
            return;
        }

        if let Some(name) = button_name(code) {
            if value == 0 || value == 1 {
                if value == 1 {
                    let now = Instant::now();
                    if self.last_press.get(&code).is_some_and(|last| now.duration_since(*last) < Duration::from_millis(60)) {
                        return;
                    }
                    self.last_press.insert(code, now);
                }
                out.push(InputEvent::Button(name, value == 1));
            }
            return;
        }

        if value != 1 { return; }
        if let Some(name) = special_name(code) {
            out.push(InputEvent::Key(name.to_string(), self.modifiers.iter().cloned().collect(), value));
            return;
        }
        if let Some((plain, shifted)) = char_pair(code) {
            let key = if self.modifiers.contains("shift") { shifted } else { plain };
            out.push(InputEvent::Key(key.to_string(), self.modifiers.iter().cloned().collect(), value));
        }
    }
}

fn modifier_name(code: u16) -> Option<&'static str> {
    match code {
        29 | 97 => Some("ctrl"),
        56 => Some("alt"),
        100 => Some("altgr"),
        42 | 54 => Some("shift"),
        125 | 126 => Some("super"),
        _ => None,
    }
}

fn button_name(code: u16) -> Option<&'static str> {
    match code {
        BTN_LEFT => Some("left"),
        BTN_RIGHT => Some("right"),
        BTN_MIDDLE => Some("middle"),
        BTN_SIDE => Some("side"),
        BTN_EXTRA => Some("extra"),
        _ => None,
    }
}

fn special_name(code: u16) -> Option<&'static str> {
    match code {
        14 => Some("BackSpace"),
        111 => Some("Delete"),
        28 | 96 => Some("Return"),
        1 => Some("Escape"),
        15 => Some("Tab"),
        105 => Some("Left"),
        106 => Some("Right"),
        103 => Some("Up"),
        108 => Some("Down"),
        102 => Some("Home"),
        107 => Some("End"),
        104 => Some("PageUp"),
        109 => Some("PageDown"),
        59..=68 => Some(match code { 59 => "F1", 60 => "F2", 61 => "F3", 62 => "F4", 63 => "F5", 64 => "F6", 65 => "F7", 66 => "F8", 67 => "F9", _ => "F10" }),
        87 => Some("F11"),
        88 => Some("F12"),
        _ => None,
    }
}

fn char_pair(code: u16) -> Option<(&'static str, &'static str)> {
    match code {
        2..=11 => {
            let index = (code - 2) as usize;
            const PLAIN: &str = "1234567890";
            const SHIFT: &str = "!@#$%^&*()";
            Some((&PLAIN[index..index + 1], &SHIFT[index..index + 1]))
        }
        16..=25 => {
            let index = (code - 16) as usize;
            const PLAIN: &str = "qwertyuiop";
            const SHIFT: &str = "QWERTYUIOP";
            Some((&PLAIN[index..index + 1], &SHIFT[index..index + 1]))
        }
        30..=38 => {
            let index = (code - 30) as usize;
            const PLAIN: &str = "asdfghjkl";
            const SHIFT: &str = "ASDFGHJKL";
            Some((&PLAIN[index..index + 1], &SHIFT[index..index + 1]))
        }
        44..=50 => {
            let index = (code - 44) as usize;
            const PLAIN: &str = "zxcvbnm";
            const SHIFT: &str = "ZXCVBNM";
            Some((&PLAIN[index..index + 1], &SHIFT[index..index + 1]))
        }
        57 => Some((" ", " ")),
        12 => Some(("-", "_")),
        13 => Some(("=", "+")),
        26 => Some(("[", "{")),
        27 => Some(("]", "}")),
        39 => Some((";", ":")),
        40 => Some(("'", "\"")),
        41 => Some(("`", "~")),
        51 => Some((",", "<")),
        52 => Some((".", ">")),
        53 => Some(("/", "?")),
        43 => Some(("\\", "|")),
        _ => None,
    }
}
