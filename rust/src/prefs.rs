use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::catalog::*;
use crate::paths;

pub const VERSION: &str = "1.1.0";

#[derive(Clone, Debug, Serialize)]
pub struct Settings {
    pub clear_shortcut: String,
    pub double_shift_click_through: bool,
    pub fade_seconds: i64,
    pub first_run_hint_shown: bool,
    pub ink_choice: String,
    pub interact_keeps_hand: bool,
    pub interact_key: String,
    pub pinned_styles: Vec<String>,
    pub pointer_size: f64,
    pub pointer_style: String,
    pub show_ink_button: bool,
    pub show_shadow: bool,
    pub show_toolbar: bool,
    pub sound_hands: bool,
    pub sound_laser: bool,
    pub tap_sound: String,
    pub toggle_shortcut: String,
    pub welcome_shown: bool,
    #[serde(skip)]
    pub path: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            clear_shortcut: "Ctrl+Alt+X".into(),
            double_shift_click_through: true,
            fade_seconds: 3,
            first_run_hint_shown: false,
            ink_choice: "match".into(),
            interact_keeps_hand: false,
            interact_key: "option".into(),
            pinned_styles: DEFAULT_PINNED.iter().map(|s| s.to_string()).collect(),
            pointer_size: 0.13,
            pointer_style: "lavender".into(),
            show_ink_button: true,
            show_shadow: true,
            show_toolbar: true,
            sound_hands: true,
            sound_laser: false,
            tap_sound: "thock".into(),
            toggle_shortcut: "Ctrl+Alt+H".into(),
            welcome_shown: false,
            path: None,
        }
    }
}

fn as_bool(value: &Value, default: bool) -> bool {
    match value {
        Value::Bool(v) => *v,
        Value::Number(n) => match n.as_i64() {
            Some(0) => false,
            Some(1) => true,
            _ => default,
        },
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        },
        _ => default,
    }
}

fn as_float(value: &Value, low: f64, high: f64, default: f64) -> f64 {
    let number = match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    };
    match number {
        Some(n) if n.is_finite() => n.clamp(low, high),
        _ => default,
    }
}

fn as_choice(value: &Value, choices: &[&str], default: &str) -> String {
    if let Value::String(s) = value {
        if choices.contains(&s.as_str()) {
            return s.clone();
        }
    }
    default.to_string()
}

fn as_shortcut(value: &Value, default: &str) -> String {
    if let Value::String(s) = value {
        let candidate = s.split_whitespace().collect::<Vec<_>>().join(" ");
        if (3..=64).contains(&candidate.chars().count()) {
            return candidate;
        }
    }
    default.to_string()
}

impl Settings {
    pub fn load() -> Self {
        Self::load_path(paths::config_file())
    }

    pub fn load_path(path: PathBuf) -> Self {
        let mut settings = Self::default();
        let raw: Value = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or(Value::Null);
        let map = match raw {
            Value::Object(map) => map,
            _ => {
                settings.path = Some(path);
                return settings;
            }
        };

        macro_rules! boolean {
            ($key:literal, $field:ident) => {
                if let Some(v) = map.get($key) {
                    settings.$field = as_bool(v, settings.$field);
                }
            };
        }

        macro_rules! string {
            ($key:literal, $field:ident) => {
                if let Some(v) = map.get($key) {
                    if let Value::String(s) = v {
                        settings.$field = s.clone();
                    }
                }
            };
        }

        boolean!("double_shift_click_through", double_shift_click_through);
        boolean!("first_run_hint_shown", first_run_hint_shown);
        boolean!("interact_keeps_hand", interact_keeps_hand);
        boolean!("show_ink_button", show_ink_button);
        boolean!("show_shadow", show_shadow);
        boolean!("show_toolbar", show_toolbar);
        boolean!("sound_hands", sound_hands);
        boolean!("sound_laser", sound_laser);
        boolean!("welcome_shown", welcome_shown);
        string!("toggle_shortcut", toggle_shortcut);
        string!("clear_shortcut", clear_shortcut);

        if let Some(v) = map.get("toggle_shortcut") {
            settings.toggle_shortcut = as_shortcut(v, "Ctrl+Alt+H");
        }
        if let Some(v) = map.get("clear_shortcut") {
            settings.clear_shortcut = as_shortcut(v, "Ctrl+Alt+X");
        }
        if let Some(v) = map.get("pointer_size") {
            settings.pointer_size = as_float(v, 0.08, 0.20, 0.13);
        }
        if let Some(v) = map.get("fade_seconds") {
            let candidate = match v {
                Value::Number(n) => n.as_f64().and_then(|f| f.is_finite().then_some(f as i64)),
                Value::String(s) => s.trim().parse::<i64>().ok(),
                _ => None,
            };
            if let Some(n) = candidate {
                if FADE_CHOICES.contains(&n) {
                    settings.fade_seconds = n;
                }
            }
        }
        if let Some(v) = map.get("ink_choice") {
            let choices: Vec<&str> = INK_CHOICES.iter().map(|(id, _, _)| *id).collect();
            settings.ink_choice = as_choice(v, &choices, "match");
        }
        if let Some(v) = map.get("interact_key") {
            settings.interact_key = as_choice(v, INTERACT_KEYS, "option");
        }
        if let Some(v) = map.get("tap_sound") {
            let choices: Vec<&str> = SOUNDS.iter().map(|(id, _)| *id).collect();
            settings.tap_sound = as_choice(v, &choices, "thock");
        }
        if let Some(v) = map.get("pointer_style") {
            if let Value::String(s) = v {
                let migrated = migrate_style(s);
                if is_valid_style(migrated) {
                    settings.pointer_style = migrated.to_string();
                }
            }
        }
        if let Some(v) = map.get("pinned_styles") {
            let mut pinned: Vec<String> = Vec::new();
            if let Value::Array(items) = v {
                for item in items {
                    if let Value::String(s) = item {
                        let migrated = migrate_style(s);
                        if is_valid_style(migrated) && !pinned.iter().any(|p| p == migrated) {
                            pinned.push(migrated.to_string());
                        }
                    }
                }
            }
            settings.pinned_styles = pinned;
        }
        if settings.pinned_styles.is_empty() {
            settings.pinned_styles = DEFAULT_PINNED.iter().map(|s| s.to_string()).collect();
        } else {
            settings.pinned_styles.truncate(8);
        }

        settings.path = Some(path);
        settings
    }

    pub fn save(&self, path: Option<&Path>) {
        let path = path
            .map(|p| p.to_path_buf())
            .or_else(|| self.path.clone())
            .unwrap_or_else(paths::config_file);
        paths::ensure_parent(&path);
        let mut map = Map::new();
        map.insert("clear_shortcut".into(), json!(self.clear_shortcut));
        map.insert("double_shift_click_through".into(), json!(self.double_shift_click_through));
        map.insert("fade_seconds".into(), json!(self.fade_seconds));
        map.insert("first_run_hint_shown".into(), json!(self.first_run_hint_shown));
        map.insert("ink_choice".into(), json!(self.ink_choice));
        map.insert("interact_keeps_hand".into(), json!(self.interact_keeps_hand));
        map.insert("interact_key".into(), json!(self.interact_key));
        map.insert("pinned_styles".into(), json!(self.pinned_styles));
        map.insert("pointer_size".into(), json!(self.pointer_size));
        map.insert("pointer_style".into(), json!(self.pointer_style));
        map.insert("show_ink_button".into(), json!(self.show_ink_button));
        map.insert("show_shadow".into(), json!(self.show_shadow));
        map.insert("show_toolbar".into(), json!(self.show_toolbar));
        map.insert("sound_hands".into(), json!(self.sound_hands));
        map.insert("sound_laser".into(), json!(self.sound_laser));
        map.insert("tap_sound".into(), json!(self.tap_sound));
        map.insert("toggle_shortcut".into(), json!(self.toggle_shortcut));
        map.insert("welcome_shown".into(), json!(self.welcome_shown));
        map.sort_keys();
        let text = serde_json::to_string_pretty(&map).unwrap_or_default();
        let _ = fs::write(&path, format!("{text}\n"));
    }

    pub fn set_style(&mut self, id: &str) -> bool {
        if !is_valid_style(id) {
            return false;
        }
        self.pointer_style = id.to_string();
        true
    }

    pub fn cycle_style(&mut self, step: i32) -> String {
        let count = STYLES.len() as i32;
        let index = STYLES.iter().position(|s| s.id == self.pointer_style).unwrap_or(0) as i32;
        let next = ((index + step).rem_euclid(count)) as usize;
        self.pointer_style = STYLES[next].id.to_string();
        self.pointer_style.clone()
    }

    pub fn pin(&mut self, id: &str) -> bool {
        if !is_valid_style(id) || self.pinned_styles.iter().any(|p| p == id) || self.pinned_styles.len() >= 8 {
            return false;
        }
        self.pinned_styles.push(id.to_string());
        true
    }

    pub fn unpin(&mut self, id: &str) -> bool {
        if self.pinned_styles.len() <= 1 {
            return false;
        }
        if let Some(i) = self.pinned_styles.iter().position(|p| p == id) {
            self.pinned_styles.remove(i);
            true
        } else {
            false
        }
    }

    pub fn reset(&mut self) {
        let path = self.path.clone();
        let welcome = self.welcome_shown;
        let hint = self.first_run_hint_shown;
        *self = Self::default();
        self.path = path;
        self.welcome_shown = welcome;
        self.first_run_hint_shown = hint;
    }

    pub fn fade_title(&self) -> String {
        match self.fade_seconds {
            3 => "After 3 seconds".into(),
            6 => "After 6 seconds".into(),
            10 => "After 10 seconds".into(),
            0 => "Keep until cleared".into(),
            _ => "After 3 seconds".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeState {
    pub active: bool,
    pub pid: u32,
    pub sound_on: bool,
    pub styles: Vec<StyleInfo>,
    pub strokes: usize,
    pub style: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct StyleInfo {
    pub id: &'static str,
    pub name: &'static str,
    #[serde(rename = "type")]
    pub kind: &'static str,
}

impl RuntimeState {
    pub fn build(pid: u32, active: bool, style: &str, sound_on: bool, strokes: usize) -> Self {
        Self {
            active,
            pid,
            sound_on,
            styles: STYLES.iter().map(|s| StyleInfo {
                id: s.id,
                name: s.title,
                kind: if s.is_laser() { "laser" } else { "hand" },
            }).collect(),
            strokes,
            style: style.to_string(),
            version: VERSION.into(),
        }
    }

    pub fn write(&self) {
        let path = paths::state_file();
        paths::ensure_parent(&path);
        let tmp = path.with_extension("tmp");
        let mut map = Map::new();
        map.insert("active".into(), json!(self.active));
        map.insert("pid".into(), json!(self.pid));
        map.insert("sound_on".into(), json!(self.sound_on));
        map.insert("styles".into(), serde_json::to_value(&self.styles).unwrap_or(json!([])));
        map.insert("strokes".into(), json!(self.strokes));
        map.insert("style".into(), json!(self.style));
        map.insert("version".into(), json!(self.version));
        map.sort_keys();
        let text = serde_json::to_string_pretty(&map).unwrap_or_default();
        if fs::write(&tmp, format!("{text}\n")).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
}

pub fn remove_state() {
    let _ = fs::remove_file(paths::state_file());
}
