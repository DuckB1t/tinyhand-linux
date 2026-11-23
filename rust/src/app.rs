use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cairo::Context;
use nix::errno::Errno;
use nix::fcntl::{Flock, FlockArg};
use nix::poll::{ppoll, PollFd, PollFlags};
use nix::sys::time::TimeSpec;
use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, Signal};
use serde_json::{json, Value};
use wait_timeout::ChildExt;

use crate::catalog::*;
use crate::draw::{clear_surface, draw_laser, draw_stroke, text, HandRenderer, WHITE};
use crate::input::{InputEvent, InputManager};
use crate::paths;
use crate::prefs::{RuntimeState, Settings, VERSION};
use crate::ui::{Rect, UI};
use crate::wayland::Canvas;

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_signal(_: i32) {
    STOP.store(true, Ordering::SeqCst);
}

fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64()
}

#[derive(Clone)]
struct Stroke {
    points: Vec<(f64, f64, f64)>,
    color: (f64, f64, f64),
    arrow: bool,
}

#[derive(Clone, Copy)]
struct Ripple {
    x: f64,
    y: f64,
    born: f64,
}

pub struct App {
    prefs: Settings,
    renderer: HandRenderer,
    ui: UI,
    canvas: Option<Canvas>,
    input: Option<InputManager>,
    control: Option<UnixListener>,
    lock_file: Option<Flock<File>>,

    running: bool,
    pub active: bool,
    sound_on: bool,
    cursor: (f64, f64),
    grip_x: Option<f64>,
    press: f64,
    drawing: bool,
    strokes: Vec<Stroke>,
    ripples: Vec<Ripple>,
    modifiers: HashSet<String>,
    interacting: bool,
    guard_until: f64,
    click_through: bool,
    shift_taps: Vec<f64>,
    reveal_until: f64,
    hint_until: f64,
    notice: String,
    notice_until: f64,
    settings_open: bool,
    welcome_open: bool,
    last_input_rects: Vec<(i32, i32, i32, i32)>,
}

impl App {
    pub fn new() -> Self {
        let prefs = Settings::load();
        let sound_on = prefs.sound_hands;
        let welcome_open = !prefs.welcome_shown;
        let mut ui = UI::default();
        ui.autostart = paths::autostart_file().exists();
        ui.fields.insert("hand_path".into(), String::new());
        ui.welcome_open = welcome_open;
        let renderer = HandRenderer::new();
        Self {
            prefs, renderer, ui,
            canvas: None, input: None, control: None, lock_file: None,
            running: true, active: true, sound_on, cursor: (0.0, 0.0),
            grip_x: None, press: 0.0, drawing: false, strokes: Vec::new(), ripples: Vec::new(),
            modifiers: HashSet::new(), interacting: false, guard_until: 0.0, click_through: false,
            shift_taps: Vec::new(), reveal_until: 0.0, hint_until: 0.0,
            notice: String::new(), notice_until: 0.0, settings_open: false, welcome_open,
            last_input_rects: Vec::new(),
        }
    }

    pub fn apply_startup_flags(&mut self, hidden: bool, settings: bool) {
        if hidden {
            self.active = false;
            self.welcome_open = false;
            self.ui.welcome_open = false;
        }
        if settings {
            self.settings_open = true;
            self.ui.settings_open = true;
        }
    }

    fn acquire_lock(&mut self) -> Result<bool, String> {
        let path = paths::lock_file();
        paths::ensure_parent(&path);
        let file = OpenOptions::new().create(true).read(true).write(true).mode(0o600).open(&path)
            .map_err(|e| e.to_string())?;
        match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
            Ok(lock) => { self.lock_file = Some(lock); Ok(true) }
            Err((_, Errno::EWOULDBLOCK)) => Ok(false),
            Err((_, e)) => Err(e.to_string()),
        }
    }

    fn setup_control(&mut self) -> Result<(), String> {
        let path = paths::control_socket();
        paths::ensure_parent(&path);
        let _ = fs::remove_file(&path);
        let listener = UnixListener::bind(&path).map_err(|e| e.to_string())?;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        self.control = Some(listener);
        Ok(())
    }

    fn log(&self, message: &str) {
        let text = format!("{} [{}] {}\n", chrono_stamp(), std::process::id(), message);
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(paths::log_file()) {
            let _ = file.write_all(text.as_bytes());
        }
        println!("[tinyhand] {message}");
    }

    fn write_state(&self) {
        RuntimeState::build(std::process::id(), self.active, &self.prefs.pointer_style, self.sound_on, self.strokes.len()).write();
    }

    fn activate(&mut self, first: bool) {
        self.active = true;
        self.reveal_until = now() + 3.0;
        if first && !self.prefs.first_run_hint_shown {
            self.prefs.first_run_hint_shown = true;
            self.prefs.save(None);
            self.reveal_until = now() + 10.0;
            self.hint_until = now() + 10.0;
            self.ui.hint_parts = vec![
                ("Click".into(), "to tap".into()),
                ("Drag".into(), "to draw".into()),
                ("Hold ⌥".into(), "to click for real".into()),
                ("Esc".into(), "to exit".into()),
            ];
        }
        self.write_state();
    }

    fn deactivate(&mut self) {
        self.active = false;
        self.drawing = false;
        self.strokes.clear();
        self.ripples.clear();
        self.ui.hint_parts.clear();
        self.ui.ink_open = false;
        self.settings_open = false;
        self.welcome_open = false;
        self.ui.settings_open = false;
        self.ui.welcome_open = false;
        self.press = 0.0;
        self.write_state();
    }

    fn toggle(&mut self) {
        if self.active { self.deactivate(); } else { self.activate(false); }
    }

    fn notify(&mut self, message: &str) {
        self.notice = message.to_string();
        self.notice_until = now() + 1.6;
        self.reveal_until = self.reveal_until.max(now() + 1.6);
        self.set_dirty();
    }

    fn set_dirty(&mut self) {
        if let Some(canvas) = &mut self.canvas { canvas.set_dirty(true); }
    }

    fn open_settings(&mut self, section: Option<String>) {
        self.settings_open = true;
        self.welcome_open = false;
        self.ui.settings_open = true;
        self.ui.welcome_open = false;
        if let Some(section) = section {
            if !section.is_empty() { self.ui.settings_section = section; }
        }
        self.ui.scroll_y = 0.0;
        self.set_dirty();
    }

    fn play_tap(&self, style_id: &str) {
        if style_id == "laser" && !self.prefs.sound_laser { return; }
        if !self.sound_on { return; }
        let sample = if SOUNDS.iter().any(|(id, _)| *id == self.prefs.tap_sound) { &self.prefs.tap_sound } else { "thock" };
        let path = paths::asset_dir().join("sounds").join(format!("tap_{sample}.wav"));
        if !path.exists() { return; }
        let _ = Command::new("paplay").arg(path).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    }

    fn cycle_style(&mut self, step: i32) {
        let id = self.prefs.cycle_style(step);
        self.prefs.save(None);
        self.set_dirty();
        self.notify(&format!("style: {}", style_title(&id)));
    }

    pub fn command(&mut self, text: &str) -> Result<Value, String> {
        let normalized = text.replace('/', " ");
        let parts: Vec<&str> = normalized.split_whitespace().collect();
        let cmd = parts.first().map(|s| s.to_ascii_lowercase()).unwrap_or_else(|| "status".into());
        let arg = parts.get(1).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
        match cmd.as_str() {
            "on" => { self.active = true; self.activate(false); }
            "off" => self.deactivate(),
            "toggle" => self.toggle(),
            "clear" => {
                self.strokes.clear(); self.ripples.clear(); self.notify("drawing cleared");
            }
            "demo" => {
                self.active = true;
                self.strokes.clear();
                self.ripples.clear();
                let (cx, cy) = self.cursor;
                let points = (0..60).map(|i| {
                    let a = i as f64 * 2.0 * std::f64::consts::PI / 59.0;
                    (cx + 150.0 * a.cos(), cy + 95.0 * a.sin(), now())
                }).collect();
                self.strokes.push(Stroke { points, color: (0.15, 0.6, 1.0), arrow: false });
                self.ripples.push(Ripple { x: cx, y: cy, born: now() });
                self.notify("demo stroke");
            }
            "quit" => self.running = false,
            "style" => {
                if arg.is_empty() || arg == "next" {
                    self.cycle_style(1);
                } else if arg == "prev" {
                    self.cycle_style(-1);
                } else if is_valid_style(&arg) {
                    self.prefs.pointer_style = arg.clone();
                    self.set_dirty();
                    self.notify(&format!("style: {}", style_title(&arg)));
                } else {
                    self.notify(&format!("style: {}", style_title(&self.prefs.pointer_style.clone())));
                }
            }
            "sound" => {
                self.sound_on = match arg.as_str() {
                    "on" => true,
                    "off" => false,
                    _ => !self.sound_on,
                };
                self.prefs.sound_hands = self.sound_on;
                self.notify(if self.sound_on { "sound: on" } else { "sound: off" });
            }
            "settings" => self.open_settings(Some(arg)),
            "section" => self.open_settings(Some(arg)),
            "toolbar" => self.reveal_until = now() + 10.0,
            "welcome" => {
                self.welcome_open = true;
                self.ui.welcome_open = true;
            }
            "status" => {}
            _ => return Err(format!("unknown command: {cmd}")),
        }
        self.prefs.save(None);
        self.write_state();
        self.set_dirty();
        Ok(self.status())
    }

    fn status(&self) -> Value {
        let reveal = (self.reveal_until - now()).max(0.0);
        json!({
            "active": self.active,
            "style": self.prefs.pointer_style,
            "sound": self.sound_on,
            "strokes": self.strokes.len(),
            "cursor": [self.cursor.0.round_ties_even() as i64, self.cursor.1.round_ties_even() as i64],
            "settings": self.settings_open,
            "toolbar": self.ui.toolbar_visible,
            "reveal": (reveal * 10.0).round_ties_even() / 10.0,
            "show_toolbar": self.prefs.show_toolbar,
            "widgets": self.ui.widgets.len(),
            "version": VERSION,
        })
    }

    fn set_cursor(&mut self, x: f64, y: f64) {
        if self.cursor == (x, y) { return; }
        self.cursor = (x, y);
        if self.drawing { self.add_stroke_point(); }
        self.set_dirty();
    }

    fn add_stroke_point(&mut self) {
        if !self.drawing || self.strokes.is_empty() { return; }
        let (x, y) = self.cursor;
        let last = self.strokes.last().unwrap().points.last().copied();
        if last.is_none_or(|p| (p.0 - x).hypot(p.1 - y) > 1.5) {
            if let Some(stroke) = self.strokes.last_mut() {
                stroke.points.push((x, y, now()));
            }
        }
    }

    fn on_button(&mut self, button: &'static str, pressed: bool) {
        let x = self.cursor.0;
        let y = self.cursor.1;
        let capture = !self.ui.widgets.is_empty() && (self.settings_open || self.welcome_open || self.ui.toolbar_visible);
        if capture && pressed {
            if let Some(index) = self.ui.hit(x, y).map(|w| w.id.clone()) {
                let widget = self.ui.widgets.iter().find(|w| w.id == index).unwrap();
                let value = widget.value.clone();
                let kind = widget.kind.clone();
                if kind == "slider" {
                    self.set_slider(&index, x);
                } else {
                    self.ui_action(&index, &value);
                }
                return;
            }
        }
        if !self.active || (!pressed && button != "left") { return; }
        if button == "shift" { return; }
        if matches!(button, "right" | "middle" | "side" | "extra") {
            if button == "right" && pressed { self.cycle_style(1); }
            return;
        }
        if button != "left" || self.click_through { return; }
        if pressed && !self.drawing {
            self.drawing = true;
            let color = resolved_ink(&self.prefs.ink_choice, &self.prefs.pointer_style);
            self.strokes.push(Stroke { points: Vec::new(), color, arrow: self.modifiers.contains("ctrl") });
            self.add_stroke_point();
            self.ripples.push(Ripple { x, y, born: now() });
            self.press = 1.0;
            let style = self.prefs.pointer_style.clone();
            self.play_tap(&style);
            self.ui.hint_parts.clear();
        } else if !pressed && self.drawing {
            self.drawing = false;
            self.press = 0.0;
        }
    }

    fn set_slider(&mut self, id: &str, x: f64) {
        let widget = self.ui.widgets.iter().find(|w| w.id == id);
        let Some(widget) = widget else { return; };
        if widget.w() <= 0.0 { return; }
        let ratio = ((x - widget.x()) / widget.w()).clamp(0.0, 1.0);
        let size = ((0.08 + ratio * 0.12) * 1000.0).round() / 1000.0;
        self.prefs.pointer_size = size;
        self.prefs.save(None);
        self.notify(&format!("pointer size: {size:.2}"));
    }

    fn matches_shortcut(key: &str, modifiers: &HashSet<String>, shortcut: &str) -> bool {
        let mut expected = HashSet::new();
        let mut main: Option<String> = None;
        for part in shortcut.split('+') {
            let part = part.trim().to_ascii_lowercase();
            if part.is_empty() { continue; }
            let alias = match part.as_str() {
                "ctrl" | "control" => Some("ctrl"),
                "alt" | "option" => Some("alt"),
                "shift" => Some("shift"),
                "super" | "cmd" | "command" | "meta" => Some("super"),
                _ => None,
            };
            if let Some(alias) = alias { expected.insert(alias.to_string()); } else { main = Some(part); }
        }
        match main {
            Some(main) => key.to_ascii_lowercase() == main && *modifiers == expected,
            None => false,
        }
    }

    fn on_key(&mut self, key: String, modifiers: HashSet<String>, value: i32) {
        self.modifiers = modifiers.clone();
        if matches!(key.as_str(), "shift" | "ctrl" | "alt" | "super") {
            self.interaction_modifiers();
            return;
        }
        if value != 1 { return; }
        if let Some(field) = self.ui.recording.clone() {
            self.record_shortcut(&key, &modifiers, &field);
            return;
        }
        if let Some(field) = self.ui.active_field.clone() {
            if self.ui.fields.contains_key(&field) {
                self.type_into_field(&field, &key);
                return;
            }
        }
        if key == "Escape" {
            if self.ui.picker_open { self.ui.picker_open = false; }
            else if self.settings_open { self.settings_open = false; self.ui.settings_open = false; self.ui.picker_open = false; }
            else if self.welcome_open { self.welcome_open = false; self.ui.welcome_open = false; }
            else if self.active { self.deactivate(); }
            self.set_dirty();
            return;
        }
        if Self::matches_shortcut(&key, &modifiers, &self.prefs.toggle_shortcut) {
            self.toggle();
        } else if Self::matches_shortcut(&key, &modifiers, &self.prefs.clear_shortcut) {
            self.strokes.clear(); self.ripples.clear(); self.notify("drawing cleared");
        } else if key == "e" && modifiers == HashSet::from(["ctrl".to_string(), "alt".to_string()]) {
            self.cycle_style(1);
        } else if key == "m" && modifiers == HashSet::from(["ctrl".to_string(), "alt".to_string()]) {
            self.sound_on = !self.sound_on;
            self.prefs.sound_hands = self.sound_on;
            self.notify(if self.sound_on { "sound: on" } else { "sound: off" });
        } else if key == "s" && modifiers == HashSet::from(["ctrl".to_string(), "alt".to_string()]) {
            self.open_settings(None);
        }
    }

    fn record_shortcut(&mut self, key: &str, modifiers: &HashSet<String>, which: &str) {
        if key == "Escape" { self.ui.recording = None; return; }
        let mut parts = Vec::new();
        for (name, present) in [("Ctrl", "ctrl"), ("Alt", "alt"), ("Shift", "shift"), ("Super", "super")] {
            if modifiers.contains(present) { parts.push(name.to_string()); }
        }
        if !modifiers.iter().any(|m| m == key) { parts.push(key.to_string()); }
        let combo = parts.join("+");
        if which == "toggle_shortcut" { self.prefs.toggle_shortcut = combo.clone(); }
        else if which == "clear_shortcut" { self.prefs.clear_shortcut = combo.clone(); }
        self.ui.recording = None;
        self.prefs.save(None);
        self.notify(&format!("shortcut: {combo}"));
    }

    fn type_into_field(&mut self, field: &str, key: &str) {
        let current = self.ui.fields.get(field).cloned().unwrap_or_default();
        if key == "BackSpace" {
            let mut chars: Vec<char> = current.chars().collect();
            chars.pop();
            self.ui.fields.insert(field.to_string(), chars.into_iter().collect());
        } else if key == "Return" || key == "Escape" {
            self.ui.active_field = None;
        } else if key.chars().count() == 1 {
            self.ui.fields.insert(field.to_string(), format!("{current}{key}"));
        }
    }

    fn interaction_modifiers(&mut self) {
        let held = (self.prefs.interact_key == "option" && self.modifiers.contains("alt"))
            || (self.prefs.interact_key == "control" && self.modifiers.contains("ctrl"))
            || (self.prefs.interact_key == "command" && self.modifiers.contains("super"));
        if held == self.interacting { return; }
        self.interacting = held;
        if held { self.guard_until = now() + 2.0; }
        if self.prefs.double_shift_click_through && self.modifiers.contains("shift") {
            let t = now();
            self.shift_taps.push(t);
            self.shift_taps.retain(|tap| t - *tap < 0.35);
            let taps: Vec<f64> = self.shift_taps.iter().rev().take(2).copied().collect();
            if self.shift_taps.len() >= 2 && taps.len() == 2 && taps[0] - taps[1] > 0.0 && taps[0] - taps[1] < 0.35 {
                self.click_through = !self.click_through;
                self.shift_taps.clear();
                self.notify(if self.click_through { "clicks go through" } else { "taps restored" });
            }
        }
        self.set_dirty();
    }

    fn ui_action(&mut self, id: &str, _value: &str) {
        if id == "redraw" { self.set_dirty(); return; }
        if let Some(style_id) = id.strip_prefix("style:") {
            if is_valid_style(style_id) { self.prefs.pointer_style = style_id.to_string(); self.ui.ink_open = false; }
        } else if let Some(choice) = id.strip_prefix("ink_choice:") {
            self.prefs.ink_choice = choice.to_string(); self.ui.ink_open = false;
        } else if let Some(choice) = id.strip_prefix("settings_ink:") {
            self.prefs.ink_choice = choice.to_string();
        } else if let Some(sound) = id.strip_prefix("sound:") {
            if sound == "off" {
                self.sound_on = false; self.prefs.sound_hands = false;
            } else {
                self.sound_on = true; self.prefs.sound_hands = true; self.prefs.tap_sound = sound.to_string();
                let sample = sound.to_string();
                self.play_tap(&sample);
            }
        } else if let Some(section) = id.strip_prefix("section:") {
            self.ui.settings_section = section.to_string(); self.ui.scroll_y = 0.0; self.ui.picker_open = false;
        } else if let Some(style_id) = id.strip_prefix("pinned:") {
            self.prefs.pointer_style = style_id.to_string();
        } else if let Some(style_id) = id.strip_prefix("picker_style:") {
            if self.prefs.set_style(style_id) { let _ = self.prefs.pin(style_id); }
            self.ui.picker_open = false;
        } else if let Some(field) = id.strip_prefix("field:") {
            if field == "toggle_shortcut" || field == "clear_shortcut" {
                self.ui.active_field = None; self.ui.recording = Some(field.to_string());
            } else {
                self.ui.active_field = Some(field.to_string());
            }
        } else if let Some(link) = id.strip_prefix("link:") {
            let urls = ["https://tinyhand.app", "https://github.com/nilbuild/tinyhand", "https://x.com/nilbuild", "https://github.com/nilbuild/tinyhand/blob/main/LICENSE"];
            if let Ok(index) = link.parse::<usize>() {
                if let Some(url) = urls.get(index) {
                    let _ = Command::new("xdg-open").arg(url).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
                }
            }
        } else {
            match id {
                "ink" => self.ui.ink_open = !self.ui.ink_open,
                "sound" => { self.sound_on = !self.sound_on; self.prefs.sound_hands = self.sound_on; let style = self.prefs.pointer_style.clone(); self.play_tap(&style); }
                "exit" => self.deactivate(),
                "settings_close" => { self.settings_open = false; self.ui.settings_open = false; self.ui.picker_open = false; }
                "double_shift" => self.prefs.double_shift_click_through = !self.prefs.double_shift_click_through,
                "show_shadow" => self.prefs.show_shadow = !self.prefs.show_shadow,
                "show_toolbar" => self.prefs.show_toolbar = !self.prefs.show_toolbar,
                "show_ink_button" => self.prefs.show_ink_button = !self.prefs.show_ink_button,
                "sound_laser" => self.prefs.sound_laser = !self.prefs.sound_laser,
                "autostart" => { self.ui.autostart = !self.ui.autostart; self.set_autostart(self.ui.autostart); }
                "welcome_shown" => self.prefs.welcome_shown = !self.prefs.welcome_shown,
                "interact_key" => {
                    let index = INTERACT_KEYS.iter().position(|k| *k == self.prefs.interact_key).unwrap_or(0);
                    self.prefs.interact_key = INTERACT_KEYS[(index + 1) % INTERACT_KEYS.len()].to_string();
                }
                "interact_keeps_hand" => self.prefs.interact_keeps_hand = !self.prefs.interact_keeps_hand,
                "fade" => {
                    let index = FADE_CHOICES.iter().position(|v| *v == self.prefs.fade_seconds).unwrap_or(0);
                    self.prefs.fade_seconds = FADE_CHOICES[(index + 1) % FADE_CHOICES.len()];
                }
                "reset_toolbar" => self.prefs.pinned_styles = DEFAULT_PINNED.iter().map(|s| s.to_string()).collect(),
                "add_pointer" => { self.ui.picker_open = !self.ui.picker_open; self.ui.picker_scroll = 0.0; }
                "picker_make" => { self.ui.picker_open = false; self.ui.settings_section = "make_hand".into(); self.ui.scroll_y = 0.0; }
                "make_hand" => { self.ui.settings_section = "make_hand".into(); self.ui.scroll_y = 0.0; }
                "back_pointers" => { self.ui.settings_section = "pointers".into(); self.ui.scroll_y = 0.0; }
                "copy_prompt" => self.copy_prompt(),
                "add_hand" => self.notify("custom-hand import is available in the docs"),
                "reset_all" => { self.prefs.reset(); self.sound_on = self.prefs.sound_hands; self.notify("settings reset"); }
                "welcome_button" => {
                    if !self.prefs.welcome_shown { self.prefs.welcome_shown = true; self.open_settings(Some("general".into())); }
                    else { self.welcome_open = false; self.ui.welcome_open = false; self.activate(false); }
                }
                "welcome_close" => { self.prefs.welcome_shown = true; self.welcome_open = false; self.ui.welcome_open = false; }
                _ => {}
            }
        }
        self.prefs.save(None);
        self.write_state();
        self.set_dirty();
    }

    fn set_autostart(&self, enabled: bool) {
        let path = paths::autostart_file();
        if !enabled {
            let _ = fs::remove_file(path);
            return;
        }
        if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
        let venv = paths::data_dir().join("venv/bin/tinyhand");
        let executable = if venv.exists() { venv.to_string_lossy().to_string() }
            else { std::env::var("PATH").ok().and_then(|p| p.split(':').map(PathBuf::from).find(|d| {
                let candidate = d.join("tinyhand");
                candidate.is_file() && fs::metadata(&candidate).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
            }).map(|d| d.join("tinyhand").to_string_lossy().to_string())).unwrap_or_else(|| "tinyhand".into()) };
        let content = format!("[Desktop Entry]\nType=Application\nName=TinyHand Linux\nExec={executable} --hidden\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n");
        let _ = fs::write(path, content);
    }

    fn copy_prompt(&mut self) {
        let idea = self.ui.fields.get("idea").cloned().unwrap_or_default();
        let idea = if idea.is_empty() { "a soft vinyl toy in a bright colour" } else { &idea };
        let prompt = format!("Make a pointer image for TinyHand. Draw one cartoon right hand in this style: {idea}. We see the back of the hand; index finger points straight up at the far left; transparent background; upright; portrait; no shadow, text or stick.");
        if let Ok(mut child) = Command::new("xclip").args(["-selection", "clipboard"]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            if let Some(mut stdin) = child.stdin.take() { let _ = stdin.write_all(prompt.as_bytes()); }
            match child.wait_timeout(Duration::from_secs(1)) {
                Ok(Some(_)) => {}
                Ok(None) => { let _ = child.kill(); let _ = child.wait(); }
                Err(_) => { let _ = child.kill(); let _ = child.wait(); }
            }
        }
        self.notify("prompt copied");
    }

    fn handle_control(&mut self) {
        if self.control.is_none() { return; }
        let accepted = self.control.as_ref().and_then(|server| server.accept().ok());
        let Some((mut conn, _)) = accepted else {
            self.log("control error: accept failed");
            return;
        };
        let _ = conn.set_read_timeout(Some(Duration::from_millis(1500)));
        let mut data = [0u8; 4096];
        let text = match conn.read(&mut data) {
            Ok(n) => String::from_utf8_lossy(&data[..n]).trim().to_string(),
            Err(e) => { self.log(&format!("control error: {e}")); return; }
        };
        let result = self.command(&text);
        let response = match result {
            Ok(value) => py_json(&value),
            Err(error) => py_json(&json!({ "error": error })),
        };
        if let Err(e) = conn.write_all(format!("{response}\n").as_bytes()) {
            self.log(&format!("control error: {e}"));
        }
    }

    fn tick(&mut self) {
        let t = now();
        self.press *= if self.drawing { 0.94 } else { 0.82 };
        let (width, height) = self.canvas.as_ref().map(|c| c.size()).unwrap_or((0, 0));
        let target = (self.cursor.0 + 0.30 * width as f64).clamp(0.15 * width as f64, 1.05 * width as f64);
        match self.grip_x {
            None => self.grip_x = Some(target),
            Some(grip) => self.grip_x = Some(grip + (target - grip) * 0.08),
        }
        let near_toolbar = self.cursor.1 > height as f64 - 170.0;
        let toolbar = self.active && self.prefs.show_toolbar && (near_toolbar || t < self.reveal_until || self.ui.ink_open || !self.ui.hint_parts.is_empty());
        self.ui.toolbar_visible = toolbar;
        self.ui.settings_open = self.settings_open;
        self.ui.welcome_open = self.welcome_open;
        if !self.ui.hint_parts.is_empty() && t > self.hint_until { self.ui.hint_parts.clear(); }
        if t > self.notice_until { self.notice.clear(); }
        self.ui.ink_open = self.ui.ink_open && toolbar && self.prefs.show_ink_button;
        self.ui.layout(&self.prefs, width as f64, height as f64);
        let rects = if self.settings_open || self.welcome_open || toolbar { self.ui.input_rects.clone() } else { Vec::new() };
        if rects != self.last_input_rects {
            self.last_input_rects = rects.clone();
            if let Some(canvas) = &mut self.canvas { canvas.set_interactive(!rects.is_empty(), &rects); }
        }
    }

    fn render(&mut self) {
        let t = now();
        self.ripples.retain(|ripple| t - ripple.born < 0.45);
        let animating = !self.ripples.is_empty() || self.drawing || self.prefs.pointer_style == "laser" || self.press > 0.01;
        let dirty = self.canvas.as_ref().is_some_and(|c| c.dirty());
        if !(dirty || animating) { return; }
        self.strokes.retain(|stroke| {
            if self.prefs.fade_seconds <= 0 || stroke.points.is_empty() { return true; }
            let age = t - stroke.points.last().unwrap().2;
            let delay = (self.prefs.fade_seconds as f64).max(0.5);
            age <= delay || (1.0 - (age - delay) / 0.6) > 0.0
        });
        let width = self.canvas.as_ref().map(|c| c.size().0).unwrap_or(0);
        let height = self.canvas.as_ref().map(|c| c.size().1).unwrap_or(0);
        let ui = &self.ui;
        let renderer = &self.renderer;
        let prefs = &self.prefs;
        let active = self.active;
        let strokes = &self.strokes;
        let ripples = &self.ripples;
        let press = self.press;
        let grip_x = self.grip_x;
        let cursor = self.cursor;
        let interacting = self.interacting;
        let click_through = self.click_through;
        let guard_until = self.guard_until;
        let notice = &self.notice;
        let settings_open = self.settings_open;
        let welcome_open = self.welcome_open;
        let canvas = self.canvas.as_mut().unwrap();
        canvas.draw(|ctx, _w, _h| {
            draw_scene(ctx, width, height, active, strokes, ripples, t, prefs, renderer, cursor, grip_x, press, interacting, click_through, guard_until);
            ui.paint(ctx, renderer, prefs);
            if !notice.is_empty() && !settings_open && !welcome_open {
                text(ctx, width as f64 / 2.0, 54.0, notice, 14.0, (1.0, 1.0, 1.0, 0.95), true, "center");
            }
        });
    }

    pub fn run(&mut self) -> i32 {
        match self.acquire_lock() {
            Ok(true) => {}
            Ok(false) => {
                eprintln!("tinyhand: another instance is already running");
                return 0;
            }
            Err(e) => {
                eprintln!("tinyhand: {e}");
                return 1;
            }
        }
        self.canvas = match Canvas::new() { Ok(canvas) => Some(canvas), Err(e) => { eprintln!("tinyhand: {e}"); return 1; } };
        if let Err(e) = self.setup_control() { eprintln!("tinyhand: control socket: {e}"); return 1; }
        self.input = Some(InputManager::new());
        if self.active {
            let first = !self.prefs.first_run_hint_shown;
            self.activate(first);
        }
        let action = SigAction::new(SigHandler::Handler(handle_signal), SaFlags::empty(), nix::sys::signal::SigSet::empty());
        unsafe {
            let _ = sigaction(Signal::SIGINT, &action);
            let _ = sigaction(Signal::SIGTERM, &action);
        }
        self.write_state();
        self.log("ready on Hyprland/Wayland");

        let frame = Duration::from_nanos(16_666_666);
        while self.running && !STOP.load(Ordering::SeqCst) {
            let mut events = Vec::new();
            if let Some(input) = &mut self.input { input.poll_hypr(&mut events); }
            self.apply_input(events);
            if let Some(canvas) = &mut self.canvas {
                if let Some((w, h)) = canvas.take_resized() {
                    self.strokes.clear();
                    self.grip_x = None;
                    self.log(&format!("surface configured: {w}x{h}"));
                }
            }
            self.tick();
            self.render();

            let wayland_fd = self.canvas.as_ref().map(|c| c.fd());
            let control_fd = self.control.as_ref().map(|c| c.as_fd().as_raw_fd());
            let input_fds = self.input.as_mut().map(|i| i.fds()).unwrap_or_default();
            let mut all_fds = Vec::new();
            if let Some(fd) = wayland_fd { all_fds.push(fd); }
            if let Some(fd) = control_fd { all_fds.push(fd); }
            all_fds.extend(input_fds.iter().copied());
            let mut pfds: Vec<PollFd> = all_fds.iter().map(|fd| PollFd::new(unsafe { BorrowedFd::borrow_raw(*fd) }, PollFlags::POLLIN)).collect();
            let timeout = TimeSpec::new(0, frame.as_nanos() as i64);
            let ready = match ppoll(&mut pfds, Some(timeout), None) { Ok(n) => n, Err(_) => break };
            if ready == 0 { continue; }
            for pfd in &pfds {
                if !pfd.revents().is_some_and(|flags| flags.contains(PollFlags::POLLIN)) { continue; }
                let fd = pfd.as_fd().as_raw_fd();
                if wayland_fd == Some(fd) {
                    if let Some(canvas) = &mut self.canvas { let _ = canvas.dispatch(); }
                } else if control_fd == Some(fd) {
                    self.handle_control();
                } else {
                    let mut events = Vec::new();
                    if let Some(input) = &mut self.input { input.handle_ready(fd, &mut events); }
                    self.apply_input(events);
                }
            }
        }
        self.cleanup();
        0
    }

    fn apply_input(&mut self, events: Vec<InputEvent>) {
        for event in events {
            match event {
                InputEvent::Move(x, y) => self.set_cursor(x, y),
                InputEvent::Button(button, pressed) => self.on_button(button, pressed),
                InputEvent::Key(key, modifiers, value) => self.on_key(key, modifiers.into_iter().collect(), value),
                InputEvent::Wheel(dx, dy) => {
                    let mut dirty = false;
                    self.ui.scroll(dx, dy, &mut dirty);
                    if dirty { self.set_dirty(); }
                }
            }
        }
    }

    fn cleanup(&mut self) {
        self.prefs.save(None);
        self.control.take();
        self.input.take();
        for path in [paths::control_socket(), paths::lock_file(), paths::state_file()] {
            let _ = fs::remove_file(path);
        }
        self.lock_file.take();
        self.canvas.take();
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_scene(
    ctx: &Context, _width: i32, height: i32, active: bool,
    strokes: &[Stroke], ripples: &[Ripple], t: f64, prefs: &Settings,
    renderer: &HandRenderer, cursor: (f64, f64), grip_x: Option<f64>, press: f64,
    interacting: bool, click_through: bool, guard_until: f64,
) {
    clear_surface(ctx);
    if !active { return; }
    let line_width = (height as f64 * 0.0055).max(3.0);
    for stroke in strokes {
        let mut alpha = 1.0;
        if prefs.fade_seconds > 0 && !stroke.points.is_empty() {
            let age = t - stroke.points.last().unwrap().2;
            let delay = (prefs.fade_seconds as f64).max(0.5);
            if age > delay { alpha = (1.0 - (age - delay) / 0.6).max(0.0); }
        }
        if alpha > 0.0 {
            let points: Vec<(f64, f64)> = stroke.points.iter().map(|p| (p.0, p.1)).collect();
            draw_stroke(ctx, &points, stroke.color, line_width, alpha, stroke.arrow);
        }
    }
    for ripple in ripples {
        let age = t - ripple.born;
        if age >= 0.45 { continue; }
        let p = age / 0.45;
        ctx.set_source_rgba(1.0, 0.85, 0.2, 0.7 * (1.0 - p));
        ctx.set_line_width(3.0 * (1.0 - p) + 1.0);
        ctx.new_path();
        ctx.arc(ripple.x, ripple.y, 8.0 + 34.0 * p, 0.0, std::f64::consts::PI * 2.0);
        let _ = ctx.stroke();
    }
    let hand_len = prefs.pointer_size * height as f64 * (1.0 - 0.06 * press);
    let grip = (grip_x.unwrap_or(cursor.0), -0.12 * height as f64);
    let hidden = (interacting && !prefs.interact_keeps_hand) || click_through || (t < guard_until && interacting);
    if !hidden {
        renderer.draw(ctx, &prefs.pointer_style, cursor, grip, hand_len, press, prefs.show_shadow);
    }
    let _ = draw_laser;
    let _ = WHITE;
    let _ = Rect;
}

fn chrono_stamp() -> String {
    let mut seconds = now() as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&mut seconds, &mut tm) }.is_null() {
        return format!("{} [{}]", now() as i64, std::process::id());
    }
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec,
    )
}

fn py_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).unwrap_or_default(),
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(py_json).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Object(map) => {
            let parts: Vec<String> = map.iter()
                .map(|(key, value)| format!("{}: {}", py_json(&Value::String(key.clone())), py_json(value)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}
