use std::f64::consts::PI;

use cairo::Context;

use crate::catalog::*;
use crate::draw::*;
use crate::prefs::Settings;

#[derive(Clone, Copy, Debug, Default)]
pub struct Rect(pub f64, pub f64, pub f64, pub f64);

#[derive(Clone, Debug, Default)]
pub struct WidgetData {
    pub area: Option<&'static str>,
    pub picker: bool,
    pub welcome: bool,
    pub secondary: bool,
    pub bold: bool,
    pub center: bool,
    pub size: Option<f64>,
    pub selected: bool,
    pub open: bool,
    pub colour: Color,
    pub icon: &'static str,
    pub field: bool,
    pub recording: bool,
}

#[derive(Clone, Debug)]
pub struct Widget {
    pub id: String,
    pub kind: String,
    pub rect: Rect,
    pub label: String,
    pub value: String,
    pub data: WidgetData,
}

impl Widget {
    pub fn x(&self) -> f64 { self.rect.0 }
    pub fn y(&self) -> f64 { self.rect.1 }
    pub fn w(&self) -> f64 { self.rect.2 }
    pub fn h(&self) -> f64 { self.rect.3 }
}

pub struct UI {
    pub widgets: Vec<Widget>,
    pub input_rects: Vec<(i32, i32, i32, i32)>,
    pub clip_rect: Option<Rect>,
    pub fields: std::collections::HashMap<String, String>,
    pub active_field: Option<String>,
    pub recording: Option<String>,
    pub scroll_y: f64,
    pub picker_open: bool,
    pub picker_scroll: f64,
    pub hint_parts: Vec<(String, String)>,
    pub ink_open: bool,
    pub toolbar_visible: bool,
    pub settings_open: bool,
    pub welcome_open: bool,
    pub settings_section: String,
    pub toolbar_rect: Rect,
    pub tray_rect: Rect,
    pub hint_rect: Rect,
    pub settings_rect: Rect,
    pub welcome_rect: Rect,
    pub picker_rect: Rect,
    pub scroll_max: f64,
    pub autostart: bool,
}

impl Default for UI {
    fn default() -> Self {
        let mut fields = std::collections::HashMap::new();
        fields.insert("idea".into(), String::new());
        fields.insert("hand_name".into(), String::new());
        fields.insert("hand_path".into(), String::new());
        Self {
            widgets: Vec::new(),
            input_rects: Vec::new(),
            clip_rect: None,
            fields,
            active_field: None,
            recording: None,
            scroll_y: 0.0,
            picker_open: false,
            picker_scroll: 0.0,
            hint_parts: Vec::new(),
            ink_open: false,
            toolbar_visible: false,
            settings_open: false,
            welcome_open: false,
            settings_section: "general".into(),
            toolbar_rect: Rect::default(),
            tray_rect: Rect::default(),
            hint_rect: Rect::default(),
            settings_rect: Rect::default(),
            welcome_rect: Rect::default(),
            picker_rect: Rect::default(),
            scroll_max: 0.0,
            autostart: false,
        }
    }
}

impl UI {
    pub fn hit(&self, x: f64, y: f64) -> Option<&Widget> {
        for widget in self.widgets.iter().rev() {
            let wx = widget.x();
            let wy = widget.y();
            if x < wx || x > wx + widget.w() || y < wy || y > wy + widget.h() {
                continue;
            }
            if !self.visible(widget) || !self.in_container(x, y, widget) {
                continue;
            }
            return Some(widget);
        }
        None
    }

    fn visible(&self, widget: &Widget) -> bool {
        if widget.data.area == Some("page") {
            if let Some(Rect(cx, cy, cw, ch)) = self.clip_rect {
                return !(widget.x() + widget.w() < cx || widget.x() > cx + cw || widget.y() + widget.h() < cy || widget.y() > cy + ch);
            }
        }
        if widget.data.picker && self.picker_open {
            let Rect(px, py, pw, ph) = self.picker_rect;
            return !(widget.x() + widget.w() < px || widget.x() > px + pw || widget.y() + widget.h() < py || widget.y() > py + ph);
        }
        true
    }

    fn in_container(&self, x: f64, y: f64, widget: &Widget) -> bool {
        if widget.data.area == Some("page") {
            if let Some(Rect(cx, cy, cw, ch)) = self.clip_rect {
                return (cx..=cx + cw).contains(&x) && (cy..=cy + ch).contains(&y);
            }
        }
        if widget.data.picker && self.picker_open {
            let Rect(px, py, pw, ph) = self.picker_rect;
            return (px..=px + pw).contains(&x) && (py..=py + ph).contains(&y);
        }
        true
    }

    pub fn scroll(&mut self, _dx: i32, dy: i32, dirty: &mut bool) {
        if self.picker_open {
            self.picker_scroll = (self.picker_scroll + dy as f64 * 40.0).max(0.0);
        } else if self.settings_open {
            self.scroll_y = (self.scroll_y + dy as f64 * 40.0).max(0.0);
        } else {
            return;
        }
        *dirty = true;
    }

    fn add(&mut self, id: impl Into<String>, kind: impl Into<String>, rect: Rect, label: impl Into<String>, value: impl Into<String>, data: WidgetData) {
        self.widgets.push(Widget {
            id: id.into(),
            kind: kind.into(),
            rect,
            label: label.into(),
            value: value.into(),
            data,
        });
    }

    pub fn layout(&mut self, prefs: &Settings, width: f64, height: f64) {
        self.widgets.clear();
        self.input_rects.clear();
        self.clip_rect = None;
        self.layout_toolbar(prefs, width, height);
        if self.settings_open {
            self.layout_settings(prefs, width, height);
        } else if self.welcome_open {
            self.layout_welcome(prefs, width, height);
        }
        if self.picker_open && self.settings_open {
            self.layout_picker(prefs);
        }
    }

    fn layout_toolbar(&mut self, prefs: &Settings, width: f64, height: f64) {
        if !self.toolbar_visible {
            return;
        }
        let mut styles = prefs.pinned_styles.clone();
        if !styles.iter().any(|s| s == &prefs.pointer_style) {
            styles.push(prefs.pointer_style.clone());
        }
        let show_ink = prefs.show_ink_button;
        let count = styles.len() + if show_ink { 4 } else { 3 };
        let panel_w = 20.0 + count as f64 * 40.0 + (count - 1) as f64 * 6.0;
        let panel_h = 56.0;
        let x = (width - panel_w) / 2.0;
        let y = height - 70.0;
        self.toolbar_rect = Rect(x, y, panel_w, panel_h);
        self.input_rects.push((x as i32, y as i32, panel_w as i32, panel_h as i32));
        let mut cx = x + 10.0;
        for style_id in &styles {
            if let Some(style) = style(style_id) {
                self.add(
                    format!("style:{style_id}"), "style", Rect(cx, y + 8.0, 40.0, 40.0),
                    style.title, style_id,
                    WidgetData { selected: style_id == &prefs.pointer_style, ..Default::default() },
                );
            }
            cx += 46.0;
        }
        self.add("divider".to_string(), "divider".to_string(), Rect(cx, y + 16.0, 1.0, 24.0), "", "", WidgetData::default());
        cx += 7.0;
        if show_ink {
            self.add(
                "ink", "ink_button", Rect(cx, y + 8.0, 40.0, 40.0), "Ink colour", &prefs.ink_choice,
                WidgetData { open: self.ink_open, ..Default::default() },
            );
            cx += 46.0;
        }
        self.add(
            "sound", "icon_button", Rect(cx, y + 8.0, 40.0, 40.0), "Tap sound",
            if prefs.sound_hands { "speaker" } else { "speaker_slash" }, WidgetData::default(),
        );
        cx += 46.0;
        self.add("exit", "icon_button", Rect(cx, y + 8.0, 40.0, 40.0), "Exit  (Esc)", "x", WidgetData::default());

        if self.ink_open {
            let swatches = INK_CHOICES.len();
            let tray_w = 12.0 + swatches as f64 * 30.0 + (swatches - 1) as f64 * 2.0;
            let tray_h = 40.0;
            let ink_x = x + 10.0 + styles.len() as f64 * 46.0 + 27.0 - tray_w / 2.0;
            let tray_y = y - 8.0 - tray_h;
            self.tray_rect = Rect(ink_x, tray_y, tray_w, tray_h);
            self.input_rects.push((ink_x as i32, tray_y as i32, tray_w as i32, tray_h as i32));
            let mut sx = ink_x + 6.0;
            for (choice_id, title, colour) in INK_CHOICES {
                self.add(
                    format!("ink_choice:{choice_id}"), "swatch", Rect(sx, tray_y + 5.0, 30.0, 30.0),
                    *title, colour.unwrap_or(""),
                    WidgetData { selected: *choice_id == prefs.ink_choice, ..Default::default() },
                );
                sx += 32.0;
            }
        } else {
            self.tray_rect = Rect::default();
        }

        if !self.hint_parts.is_empty() {
            let length: usize = self.hint_parts.iter().map(|(k, v)| k.chars().count() + v.chars().count()).sum();
            let hint_w = (260.0_f64).max((760.0_f64).min(60.0 + length as f64 * 8.0));
            let hint_h = 36.0;
            let hint_x = x + panel_w / 2.0 - hint_w / 2.0;
            let hint_y = y - 12.0 - hint_h;
            self.hint_rect = Rect(hint_x, hint_y, hint_w, hint_h);
            self.input_rects.push((hint_x as i32, hint_y as i32, hint_w as i32, hint_h as i32));
        } else {
            self.hint_rect = Rect::default();
        }
    }

    fn layout_settings(&mut self, prefs: &Settings, width: f64, height: f64) {
        self.scroll_y = self.scroll_y.min(self.scroll_max);
        let sw = 780.0;
        let sh = 560.0;
        let sx = (width - sw) / 2.0;
        let sy = (height - sh) / 2.0;
        self.settings_rect = Rect(sx, sy, sw, sh);
        self.input_rects.push((sx as i32, sy as i32, sw as i32, sh as i32));
        let sections: [(&str, &str, &str, Color); 6] = [
            ("general", "General", "gear", WHITE),
            ("pointers", "Pointers", "hand", PURPLE),
            ("appearance", "Appearance", "sparkles", (0.95, 0.45, 0.6, 1.0)),
            ("drawing", "Drawing", "pencil", (0.96, 0.58, 0.2, 1.0)),
            ("sound", "Sound", "speaker", BLUE),
            ("about", "About", "info", WHITE),
        ];
        for (i, (section, title, glyph, colour)) in sections.iter().enumerate() {
            self.add(
                format!("section:{section}"), "tab", Rect(sx + 10.0, sy + 52.0 + i as f64 * 44.0, 190.0, 38.0),
                *title, *section,
                WidgetData { icon: glyph, colour: *colour, selected: *section == self.settings_section, ..Default::default() },
            );
        }
        self.add("settings_close", "icon_button", Rect(sx + sw - 36.0, sy + 12.0, 24.0, 24.0), "Close", "x", WidgetData::default());

        let page_x = sx + 210.0;
        let page_y = sy + 48.0;
        let page_w = sw - 210.0;
        let page_h = sh - 48.0;
        self.clip_rect = Some(Rect(page_x, page_y, page_w, page_h));
        let x = page_x + 28.0;
        let w = page_w - 56.0;
        let is_sub = self.settings_section == "make_hand";
        let section_title = if is_sub {
            "Make your own hand"
        } else {
            sections.iter().find(|(id, _, _, _)| *id == self.settings_section).map(|(_, title, _, _)| *title).unwrap_or("General")
        };
        let mut y = page_y - self.scroll_y;
        if is_sub {
            self.add("back_pointers", "back", Rect(x, y, 120.0, 24.0), "Pointers", "", WidgetData::default());
            y += 30.0;
        }
        self.add("page_title", "title", Rect(x, y, page_w - 56.0, 30.0), section_title, "", WidgetData { area: Some("page"), ..Default::default() });
        y += 46.0;
        match self.settings_section.as_str() {
            "general" => y = self.general_page(prefs, x, y, w),
            "pointers" => y = self.pointers_page(prefs, x, y, w),
            "make_hand" => y = self.make_hand_page(prefs, x, y, w),
            "appearance" => y = self.appearance_page(prefs, x, y, w),
            "drawing" => y = self.drawing_page(prefs, x, y, w),
            "sound" => y = self.sound_page(prefs, x, y, w),
            "about" => y = self.about_page(prefs, x, y, w),
            _ => {}
        }
        self.scroll_max = (y + self.scroll_y - (page_y + page_h) + 60.0).max(0.0);
        self.scroll_y = self.scroll_y.min(self.scroll_max);
    }

    fn row_text(&mut self, x: f64, y: f64, w: f64, title: &str, detail: &str, height: f64) -> f64 {
        self.add(format!("row:{title}"), "text", Rect(x + 14.0, y + 8.0, w - 150.0, 18.0), title, "", WidgetData { area: Some("page"), ..Default::default() });
        if !detail.is_empty() {
            self.add(format!("detail:{title}"), "text", Rect(x + 14.0, y + 26.0, w - 150.0, 28.0), detail, "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        }
        y + height
    }

    fn switch(&mut self, id: &str, value: bool, x: f64, y: f64, w: f64, h: f64) {
        self.add(id, "switch", Rect(x, y, w, h), "", value.to_string(), WidgetData { area: Some("page"), ..Default::default() });
    }

    fn pill(&mut self, id: &str, label: &str, x: f64, y: f64, w: f64, h: f64) {
        self.add(id, "pill", Rect(x, y, w, h), label, "", WidgetData { area: Some("page"), ..Default::default() });
    }

    fn general_page(&mut self, prefs: &Settings, x: f64, mut y: f64, w: f64) -> f64 {
        self.add("session_title", "text", Rect(x + 4.0, y, w, 18.0), "Session", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        self.add("session_card", "card", Rect(x, y + 22.0, w, 52.0), "", "", WidgetData { area: Some("page"), ..Default::default() });
        self.add("session_text", "text", Rect(x + 14.0, y + 32.0, w - 28.0, 18.0), "Wayland layer shell", "", WidgetData { area: Some("page"), ..Default::default() });
        self.add("session_detail", "text", Rect(x + 14.0, y + 50.0, w - 28.0, 18.0), "Hyprland / wlroots detected. No macOS accessibility permission is needed.", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        y += 96.0;

        self.add("shortcuts_title", "text", Rect(x + 4.0, y, w, 18.0), "Shortcuts", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        self.add("shortcuts_card", "card", Rect(x, y + 22.0, w, 106.0), "", "", WidgetData { area: Some("page"), ..Default::default() });
        self.add("toggle_label", "text", Rect(x + 14.0, y + 34.0, w - 160.0, 18.0), "Turn the pointer on or off", "", WidgetData { area: Some("page"), ..Default::default() });
        self.add("toggle_clear", "text", Rect(x + 14.0, y + 86.0, w - 160.0, 18.0), "Clear the drawing", "", WidgetData { area: Some("page"), ..Default::default() });
        self.add("field:toggle_shortcut", "field", Rect(x + w - 142.0, y + 30.0, 128.0, 30.0), "", &prefs.toggle_shortcut, WidgetData { area: Some("page"), field: true, recording: self.recording.as_deref() == Some("toggle_shortcut"), ..Default::default() });
        self.add("field:clear_shortcut", "field", Rect(x + w - 142.0, y + 82.0, 128.0, 30.0), "", &prefs.clear_shortcut, WidgetData { area: Some("page"), field: true, recording: self.recording.as_deref() == Some("clear_shortcut"), ..Default::default() });
        y += 150.0;

        self.add("clicking_title", "text", Rect(x + 4.0, y, w, 18.0), "Clicking for real", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        self.add("clicking_card", "card", Rect(x, y + 22.0, w, 204.0), "", "", WidgetData { area: Some("page"), ..Default::default() });
        let entries = [
            ("Hold to use the app", "Clicks, drags and scrolls reach the app."),
            ("Double-tap Shift to click through", "Clicks reach the app and the hand stays."),
        ];
        for (i, (title, detail)) in entries.iter().enumerate() {
            let ry = y + 32.0 + i as f64 * 62.0;
            self.add(format!("row:{title}"), "text", Rect(x + 14.0, ry, w - 160.0, 18.0), *title, "", WidgetData { area: Some("page"), ..Default::default() });
            self.add(format!("detail:{title}"), "text", Rect(x + 14.0, ry + 18.0, w - 190.0, 30.0), *detail, "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
            if i == 0 {
                self.add("interact_key", "menu", Rect(x + w - 142.0, ry + 2.0, 128.0, 30.0), &prefs.interact_key, "", WidgetData { area: Some("page"), ..Default::default() });
            } else {
                self.switch("double_shift", prefs.double_shift_click_through, x + w - 72.0, ry + 2.0, 52.0, 30.0);
            }
        }
        let ry = y + 156.0;
        self.add("while_holding", "text", Rect(x + 14.0, ry, w - 160.0, 18.0), "While holding", "", WidgetData { area: Some("page"), ..Default::default() });
        let keeps = if prefs.interact_keeps_hand { "Keep the hand" } else { "Show the cursor" };
        self.add("keeps_hand", "menu", Rect(x + w - 142.0, ry - 2.0, 128.0, 30.0), keeps, "", WidgetData { area: Some("page"), ..Default::default() });
        y += 252.0;

        self.add("startup_title", "text", Rect(x + 4.0, y, w, 18.0), "Startup", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        self.add("startup_card", "card", Rect(x, y + 22.0, w, 106.0), "", "", WidgetData { area: Some("page"), ..Default::default() });
        self.row_text(x, y + 30.0, w, "Open TinyHand when you log in", "Writes a standard XDG autostart entry (off by default).", 52.0);
        self.switch("autostart", self.autostart, x + w - 132.0, y + 41.0, 120.0, 30.0);
        self.row_text(x, y + 82.0, w, "Show a welcome card on first launch", "Explains the overlay once.", 52.0);
        self.switch("welcome_shown", !prefs.welcome_shown, x + w - 132.0, y + 93.0, 120.0, 30.0);
        y + 150.0
    }

    fn pointers_page(&mut self, prefs: &Settings, x: f64, mut y: f64, w: f64) -> f64 {
        self.add("toolbar_header", "text", Rect(x, y, 180.0, 18.0), "In your toolbar", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        self.pill("reset_toolbar", "Reset", x + w - 80.0, y - 4.0, 80.0, 26.0);
        y += 30.0;
        let slot_w = (w - 20.0 + 6.0) / 8.0;
        self.add("pinned_bg", "card", Rect(x, y, w, 58.0), "", "", WidgetData { area: Some("page"), ..Default::default() });
        for i in 0..8 {
            let rect = Rect(x + 10.0 + i as f64 * slot_w, y + 4.0, slot_w - 6.0, 50.0);
            if i < prefs.pinned_styles.len() {
                let id = &prefs.pinned_styles[i];
                self.add(format!("pinned:{id}"), "toolbar_slot", rect, style_title(id), id, WidgetData { area: Some("page"), ..Default::default() });
            } else if i == prefs.pinned_styles.len() {
                self.add("add_pointer", "add_slot", rect, "", "", WidgetData { area: Some("page"), ..Default::default() });
            } else {
                self.add(format!("empty_slot:{i}"), "empty_slot", rect, "", "", WidgetData { area: Some("page"), ..Default::default() });
            }
        }
        y += 70.0;
        let helper = if prefs.pinned_styles.len() >= 8 {
            "The toolbar is full. Remove a pointer to add another. Drag to reorder."
        } else {
            "Press + to add a pointer. Drag to reorder, or out to remove."
        };
        self.add("pinned_helper", "text", Rect(x, y, w, 18.0), helper, "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        y += 34.0;
        self.add("show_toolbar_title", "text", Rect(x + 4.0, y, w, 18.0), "Toolbar", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        self.add("show_toolbar_card", "card", Rect(x, y + 22.0, w, 106.0), "", "", WidgetData { area: Some("page"), ..Default::default() });
        self.row_text(x, y + 30.0, w, "Show the toolbar", "Pops up when the pointer turns on and near the bottom of the screen.", 52.0);
        self.switch("show_toolbar", prefs.show_toolbar, x + w - 132.0, y + 41.0, 120.0, 30.0);
        self.row_text(x, y + 82.0, w, "Ink button", "Change the colour while you present.", 52.0);
        self.switch("show_ink_button", prefs.show_ink_button, x + w - 132.0, y + 93.0, 120.0, 30.0);
        y += 150.0;
        self.add("your_hands_title", "text", Rect(x + 4.0, y, w, 18.0), "Your hands", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        self.pill("make_hand", "Make your own…", x + w - 130.0, y - 4.0, 130.0, 26.0);
        self.add("your_hands_card", "card", Rect(x, y + 22.0, w, 74.0), "", "", WidgetData { area: Some("page"), ..Default::default() });
        self.add("your_hands_empty", "text", Rect(x + 14.0, y + 38.0, w - 28.0, 18.0), "Turn any idea into a hand with an image tool, then add the PNG here.", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        self.add("your_hands_hint", "text", Rect(x + 14.0, y + 58.0, w - 28.0, 18.0), "Custom hands are stored in ~/.local/share/tinyhand-linux/hands.", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        y + 120.0
    }

    fn make_hand_page(&mut self, _prefs: &Settings, x: f64, mut y: f64, w: f64) -> f64 {
        let steps = [
            ("1", "Describe it", "What should your hand look like?", Some("idea")),
            ("2", "Make the image", "Open it in an image tool, then download a transparent PNG.", None),
            ("3", "Add it", "Drop the PNG here, or paste a path below.", Some("hand_path")),
        ];
        for (number, title, detail, field_id) in steps {
            self.add(format!("step:{number}"), "step_number", Rect(x, y + 2.0, 20.0, 20.0), number, "", WidgetData { area: Some("page"), ..Default::default() });
            self.add(format!("step_title:{number}"), "text", Rect(x + 32.0, y, w - 32.0, 18.0), title, "", WidgetData { area: Some("page"), ..Default::default() });
            self.add(format!("step_detail:{number}"), "text", Rect(x + 32.0, y + 20.0, w - 32.0, 30.0), detail, "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
            y += 56.0;
            if let Some(field_id) = field_id {
                let value = self.fields.get(field_id).cloned().unwrap_or_default();
                self.add(format!("field:{field_id}"), "field", Rect(x + 32.0, y, (420.0_f64).min(w - 32.0), 32.0), "", value, WidgetData { area: Some("page"), field: true, recording: self.recording.as_deref() == Some(field_id), ..Default::default() });
                y += 46.0;
            }
        }
        self.pill("copy_prompt", "Copy prompt", x + 32.0, y, 140.0, 30.0);
        self.add("add_hand", "primary", Rect(x + 184.0, y, 120.0, 30.0), "Add hand", "", WidgetData { area: Some("page"), ..Default::default() });
        y + 70.0
    }

    fn appearance_page(&mut self, prefs: &Settings, x: f64, mut y: f64, w: f64) -> f64 {
        self.add("pointer_card_title", "text", Rect(x + 4.0, y, w, 18.0), "Pointer", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        self.add("pointer_card", "card", Rect(x, y + 22.0, w, 152.0), "", "", WidgetData { area: Some("page"), ..Default::default() });
        self.add("size_label", "text", Rect(x + 14.0, y + 34.0, 80.0, 18.0), "Size", "", WidgetData { area: Some("page"), ..Default::default() });
        self.add("size_slider", "slider", Rect(x + 100.0, y + 34.0, 180.0, 30.0), "", format!("{}", prefs.pointer_size), WidgetData { area: Some("page"), ..Default::default() });
        self.add("shadow_label", "text", Rect(x + 14.0, y + 92.0, w - 100.0, 18.0), "Shadow under the pointer", "", WidgetData { area: Some("page"), ..Default::default() });
        self.switch("show_shadow", prefs.show_shadow, x + w - 72.0, y + 88.0, 52.0, 30.0);
        y += 190.0;
        y
    }

    fn drawing_page(&mut self, prefs: &Settings, x: f64, mut y: f64, w: f64) -> f64 {
        self.add("ink_title", "text", Rect(x + 4.0, y, w, 18.0), "Ink", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        self.add("ink_card", "card", Rect(x, y + 22.0, w, 112.0), "", "", WidgetData { area: Some("page"), ..Default::default() });
        let ink_detail = if prefs.ink_choice == "match" {
            "Matches each pointer.".to_string()
        } else {
            let title = INK_CHOICES.iter().find(|(id, _, _)| *id == prefs.ink_choice).map(|(_, title, _)| *title).unwrap_or("Match pointer");
            format!("All pointers draw in {}.", title.to_lowercase())
        };
        self.add("ink_choice_label", "text", Rect(x + 14.0, y + 34.0, 160.0, 18.0), "Ink colour", "", WidgetData { area: Some("page"), ..Default::default() });
        self.add("ink_choice_detail", "text", Rect(x + 14.0, y + 52.0, 200.0, 18.0), &ink_detail, "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        let mut sx = x + 250.0;
        for (choice_id, _, colour) in INK_CHOICES {
            self.add(format!("settings_ink:{choice_id}"), "settings_swatch", Rect(sx, y + 36.0, 26.0, 26.0), "", colour.unwrap_or(""), WidgetData { area: Some("page"), selected: *choice_id == prefs.ink_choice, ..Default::default() });
            sx += 34.0;
        }
        self.add("fade_label", "text", Rect(x + 14.0, y + 86.0, 120.0, 18.0), "Lines fade", "", WidgetData { area: Some("page"), ..Default::default() });
        let detail = format!("Clear them with {}.", prefs.clear_shortcut);
        self.add("fade_detail", "text", Rect(x + 14.0, y + 104.0, 220.0, 18.0), &detail, "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        self.add("fade_menu", "menu", Rect(x + w - 152.0, y + 88.0, 140.0, 30.0), prefs.fade_title(), "", WidgetData { area: Some("page"), ..Default::default() });
        y += 160.0;
        y
    }

    fn sound_page(&mut self, prefs: &Settings, x: f64, mut y: f64, w: f64) -> f64 {
        self.add("tap_sound_title", "text", Rect(x + 4.0, y, w, 18.0), "Tap sound", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        let cols = 4usize;
        let tile_w = (w - (cols - 1) as f64 * 8.0) / cols as f64;
        let mut entries: Vec<(&str, &str)> = vec![("off", "Off")];
        entries.extend(SOUNDS.iter().copied());
        for (i, (id, title)) in entries.iter().enumerate() {
            let cx = x + (i % cols) as f64 * (tile_w + 8.0);
            let cy = y + 30.0 + (i / cols) as f64 * 76.0;
            let selected = if *id == "off" { !prefs.sound_hands } else { prefs.sound_hands && *id == prefs.tap_sound };
            self.add(format!("sound:{id}"), "sound_tile", Rect(cx, cy, tile_w, 68.0), *title, *id, WidgetData { area: Some("page"), selected, ..Default::default() });
        }
        y += 30.0 + ((entries.len() + cols - 1) / cols) as f64 * 76.0 + 10.0;
        self.add("sound_helper", "text", Rect(x, y, w, 18.0), "Click a sound to hear it. It plays each time the hand taps.", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        y += 40.0;
        self.add("laser_sound_card", "card", Rect(x, y, w, 52.0), "", "", WidgetData { area: Some("page"), ..Default::default() });
        self.add("laser_sound_label", "text", Rect(x + 14.0, y + 10.0, 180.0, 18.0), "Laser dot taps too", "", WidgetData { area: Some("page"), ..Default::default() });
        self.add("laser_sound_detail", "text", Rect(x + 14.0, y + 28.0, 240.0, 18.0), "Off by default, for quiet pointing.", "", WidgetData { area: Some("page"), secondary: true, ..Default::default() });
        self.switch("sound_laser", prefs.sound_laser, x + w - 72.0, y + 12.0, 52.0, 30.0);
        y + 90.0
    }

    fn about_page(&mut self, _prefs: &Settings, x: f64, mut y: f64, w: f64) -> f64 {
        self.add("app_icon_big", "app_icon", Rect(x, y, 72.0, 72.0), "", "", WidgetData { area: Some("page"), ..Default::default() });
        self.add("app_name", "text", Rect(x + 88.0, y + 6.0, 200.0, 24.0), "TinyHand", "", WidgetData { area: Some("page"), size: Some(18.0), bold: true, ..Default::default() });
        self.add("app_version", "text", Rect(x + 88.0, y + 32.0, 220.0, 18.0), "Version 1.1.0 (Linux)", "", WidgetData { area: Some("page"), ..Default::default() });
        self.add("app_tagline", "text", Rect(x + 88.0, y + 52.0, 320.0, 18.0), "A tiny hand for your screen. Free and open source.", "", WidgetData { area: Some("page"), ..Default::default() });
        y += 96.0;
        let links = [
            ("Website", "tinyhand.app"),
            ("Source code", "github.com/nilbuild/tinyhand"),
            ("Made by", "@nilbuild on X"),
            ("License", "MIT"),
        ];
        self.add("links_card", "card", Rect(x, y, w, 186.0), "", "", WidgetData { area: Some("page"), ..Default::default() });
        for (i, (title, detail)) in links.iter().enumerate() {
            self.add(format!("link:{i}"), "link", Rect(x, y + 8.0 + i as f64 * 44.0, w, 44.0), format!("{title}  ·  {detail}"), "", WidgetData { area: Some("page"), ..Default::default() });
        }
        y += 210.0;
        self.pill("reset_all", "Reset all settings…", x + w - 140.0, y, 140.0, 30.0);
        y + 60.0
    }

    fn layout_picker(&mut self, prefs: &Settings) {
        let pw = 400.0;
        let ph = 340.0;
        let px = self.settings_rect.0 + 310.0;
        let py = self.settings_rect.1 + 150.0;
        self.picker_rect = Rect(px, py, pw, ph);
        self.input_rects.push((px as i32, py as i32, pw as i32, ph as i32));
        self.add("picker_bg", "card", Rect(px, py, pw, ph), "", "", WidgetData { picker: true, ..Default::default() });
        let cell_w = 64.0;
        let cell_h = 58.0;
        let cols = 5usize;
        let rows = (STYLES.len() + cols - 1) / cols;
        self.picker_scroll = self.picker_scroll.min((14.0 + rows as f64 * cell_h - (ph - 44.0)).max(0.0));
        let start_y = py + 14.0 - self.picker_scroll;
        for (i, style) in STYLES.iter().enumerate() {
            let rect = Rect(px + 12.0 + (i % cols) as f64 * 76.0, start_y + (i / cols) as f64 * 58.0, cell_w, cell_h);
            if rect.1 + cell_h < py || rect.1 > py + ph - 44.0 {
                continue;
            }
            self.add(
                format!("picker_style:{}", style.id), "picker_cell", rect, style.title, style.id,
                WidgetData { picker: true, selected: style.id == prefs.pointer_style, ..Default::default() },
            );
        }
        self.add("picker_footer", "text", Rect(px + 14.0, py + ph - 34.0, pw - 28.0, 20.0), "Make your own hand…", "", WidgetData { picker: true, secondary: true, ..Default::default() });
        self.add("picker_make", "invisible", Rect(px + 14.0, py + ph - 36.0, pw - 28.0, 24.0), "", "", WidgetData { picker: true, ..Default::default() });
    }

    fn layout_welcome(&mut self, prefs: &Settings, width: f64, height: f64) {
        let w = 340.0;
        let h = 300.0;
        let x = (width - w) / 2.0;
        let y = (height - h) / 2.0;
        self.welcome_rect = Rect(x, y, w, h);
        self.input_rects.push((x as i32, y as i32, w as i32, h as i32));
        self.add("welcome_close", "icon_button", Rect(x + w - 28.0, y + 14.0, 18.0, 18.0), "Close", "x", WidgetData::default());
        self.add("welcome_icon", "app_icon", Rect(x + (w - 76.0) / 2.0, y + 26.0, 76.0, 76.0), "", "", WidgetData::default());
        let title = if prefs.welcome_shown { "You're all set" } else { "Welcome to TinyHand" };
        let subtitle = if prefs.welcome_shown {
            format!("Press {} any time to start.", prefs.toggle_shortcut)
        } else {
            "A tiny hand for your screen.".to_string()
        };
        self.add("welcome_title", "text", Rect(x + 20.0, y + 122.0, w - 40.0, 26.0), title, "", WidgetData { welcome: true, size: Some(19.0), bold: true, center: true, ..Default::default() });
        self.add("welcome_subtitle", "text", Rect(x + 20.0, y + 154.0, w - 40.0, 20.0), &subtitle, "", WidgetData { welcome: true, secondary: true, center: true, ..Default::default() });
        self.add("welcome_button", "primary", Rect(x + (w - 180.0) / 2.0, y + 190.0, 180.0, 34.0), if prefs.welcome_shown { "Start" } else { "Open Settings" }, "", WidgetData { welcome: true, ..Default::default() });
        self.add("welcome_tip", "text", Rect(x + 20.0, y + 246.0, w - 40.0, 18.0), "Wayland layer shell · Hyprland detected", "", WidgetData { welcome: true, secondary: true, center: true, ..Default::default() });
    }

    pub fn paint(&self, ctx: &Context, renderer: &HandRenderer, prefs: &Settings) {
        if self.settings_open {
            self.paint_settings_frame(ctx);
        } else if self.toolbar_visible && self.toolbar_rect.2 > 0.0 {
            self.paint_toolbar_frame(ctx);
        }
        for widget in &self.widgets {
            if widget.data.area == Some("page") || widget.data.picker || widget.data.welcome {
                continue;
            }
            self.paint_widget(ctx, renderer, prefs, widget, true);
        }
        if self.settings_open {
            let _ = ctx.save();
            let Rect(sx, sy, sw, sh) = self.settings_rect;
            ctx.rectangle(sx + 210.0, sy + 48.0, sw - 210.0, sh - 48.0);
            ctx.clip();
            ctx.set_source_rgba(0.96, 0.96, 0.97, 1.0);
            let _ = ctx.paint();
            for widget in &self.widgets {
                if widget.data.area == Some("page") {
                    self.paint_widget(ctx, renderer, prefs, widget, false);
                }
            }
            let _ = ctx.restore();
            if self.scroll_y > 0.0 {
                self.paint_scrollbar(ctx);
            }
        }
        if self.picker_open && self.settings_open {
            let Rect(px, py, pw, ph) = self.picker_rect;
            if pw > 0.0 {
                panel(ctx, px, py, pw, ph, 12.0, 0.94);
            }
            for widget in &self.widgets {
                if widget.data.picker {
                    self.paint_widget(ctx, renderer, prefs, widget, false);
                }
            }
        }
        if self.welcome_open && !self.settings_open {
            let Rect(x, y, w, h) = self.welcome_rect;
            rounded_rect(ctx, x, y, w, h, 20.0);
            ctx.set_source_rgba(0.94, 0.94, 0.95, 1.0);
            let _ = ctx.fill_preserve();
            ctx.set_source_rgba(0.0, 0.0, 0.0, 0.28);
            let _ = ctx.stroke();
            for widget in &self.widgets {
                if widget.data.welcome || widget.id.starts_with("welcome_") {
                    self.paint_widget(ctx, renderer, prefs, widget, false);
                }
            }
        }
    }

    fn paint_toolbar_frame(&self, ctx: &Context) {
        let Rect(x, y, w, h) = self.toolbar_rect;
        panel(ctx, x, y, w, h, 16.0, 0.72);
        if self.ink_open && self.tray_rect.2 > 0.0 {
            let Rect(x, y, w, h) = self.tray_rect;
            panel(ctx, x, y, w, h, 14.0, 0.72);
        }
        if !self.hint_parts.is_empty() && self.hint_rect.2 > 0.0 {
            let Rect(x, y, w, h) = self.hint_rect;
            panel(ctx, x, y, w, h, 12.0, 0.62);
        }
    }

    fn paint_settings_frame(&self, ctx: &Context) {
        let Rect(sx, sy, sw, sh) = self.settings_rect;
        if sw <= 0.0 { return; }
        panel(ctx, sx, sy, sw, sh, 18.0, 0.82);
        rounded_rect(ctx, sx, sy, 210.0, sh, 18.0);
        ctx.set_source_rgba(0.015, 0.017, 0.022, 0.42);
        let _ = ctx.fill();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.09);
        ctx.set_line_width(1.0);
        ctx.move_to(sx + 210.5, sy + 12.0);
        ctx.line_to(sx + 210.5, sy + sh - 12.0);
        let _ = ctx.stroke();
    }

    fn paint_scrollbar(&self, ctx: &Context) {
        let Rect(sx, sy, sw, sh) = self.settings_rect;
        let track_x = sx + sw - 6.0;
        let track_y = sy + 52.0;
        let track_h = sh - 60.0;
        let max_scroll = self.scroll_max.max(1.0);
        let knob_h = (28.0_f64).max(track_h * (track_h / (track_h + max_scroll)));
        let pos = track_y + (self.scroll_y / max_scroll) * (track_h - knob_h);
        rounded_rect(ctx, track_x, pos, 4.0, knob_h, 2.0);
        ctx.set_source_rgba(0.0, 0.0, 0.0, 0.2);
        let _ = ctx.fill();
    }

    fn paint_widget(&self, ctx: &Context, renderer: &HandRenderer, prefs: &Settings, widget: &Widget, dark: bool) {
        let Rect(x, y, w, h) = widget.rect;
        if w <= 0.0 || h <= 0.0 { return; }
        let fg: Color = if dark { WHITE } else { (0.11, 0.11, 0.13, 1.0) };
        let secondary: Color = if dark { SECONDARY } else { (0.42, 0.42, 0.46, 1.0) };
        let card_fill: Color = if dark { (1.0, 1.0, 1.0, 0.045) } else { (0.0, 0.0, 0.0, 0.035) };
        match widget.kind.as_str() {
            "invisible" => {}
            "divider" => {
                ctx.set_source_rgba(1.0, 1.0, 1.0, 0.18);
                ctx.rectangle(x, y, w, h);
                let _ = ctx.fill();
            }
            "card" => {
                rounded_rect(ctx, x, y, w, h, 12.0);
                ctx.set_source_rgba(card_fill.0, card_fill.1, card_fill.2, card_fill.3);
                let _ = ctx.fill();
            }
            "text" => {
                let size = widget.data.size.unwrap_or(if widget.data.secondary { 11.0 } else { 13.0 });
                let colour = if widget.data.secondary { secondary } else { fg };
                if widget.data.center {
                    text(ctx, x + w / 2.0, y + size + 2.0, &widget.label, size, colour, widget.data.bold, "center");
                } else {
                    text(ctx, x, y + size, &widget.label, size, colour, widget.data.bold, "left");
                }
            }
            "title" => {
                text(ctx, x, y + 26.0, &widget.label, 22.0, fg, true, "left");
            }
            "back" => {
                icon(ctx, "chevron_left", x + 6.0, y + 12.0, 12.0, secondary);
                text(ctx, x + 18.0, y + 16.0, &widget.label, 12.0, secondary, false, "left");
            }
            "app_icon" => app_icon(ctx, x, y, w, h),
            "icon" => icon(ctx, &widget.value, x + w / 2.0, y + h / 2.0, w.min(h) * 0.55, fg),
            "icon_button" => {
                let colour = if dark { (1.0, 1.0, 1.0, 0.85) } else { (0.2, 0.2, 0.22, 1.0) };
                icon(ctx, &widget.value, x + w / 2.0, y + h / 2.0, w.min(h) * 0.45, colour);
            }
            "style" => {
                rounded_rect(ctx, x, y, w, h, 10.0);
                ctx.set_source_rgba(1.0, 1.0, 1.0, if widget.data.selected { 0.18 } else { 0.0 });
                let _ = ctx.fill();
                if let Some(thumb) = renderer.thumbnail_for(&widget.value, 40) {
                    let _ = ctx.set_source_surface(&thumb, x, y);
                    let _ = ctx.paint();
                } else {
                    laser_icon(ctx, x + w / 2.0, y + h / 2.0, 20.0);
                }
            }
            "ink_button" => {
                rounded_rect(ctx, x, y, w, h, 10.0);
                ctx.set_source_rgba(1.0, 1.0, 1.0, if widget.data.open { 0.18 } else { 0.0 });
                let _ = ctx.fill();
                if widget.value == "match" {
                    rainbow_dot(ctx, x + w / 2.0, y + h / 2.0, 13.0);
                } else {
                    ink_dot(ctx, x + w / 2.0, y + h / 2.0, if widget.value.is_empty() { "#FFFFFF" } else { &widget.value }, 13.0);
                }
            }
            "swatch" => {
                if widget.value.is_empty() {
                    rainbow_dot(ctx, x + w / 2.0, y + h / 2.0, 13.0);
                } else {
                    ink_dot(ctx, x + w / 2.0, y + h / 2.0, &widget.value, 12.0);
                }
                if widget.data.selected {
                    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.2);
                    ctx.new_path();
                    ctx.arc(x + w / 2.0, y + h / 2.0, 15.0, 0.0, 2.0 * PI);
                    let _ = ctx.stroke();
                }
            }
            "settings_swatch" => {
                if widget.value.is_empty() {
                    rainbow_dot(ctx, x + w / 2.0, y + h / 2.0, 9.0);
                } else {
                    ink_dot(ctx, x + w / 2.0, y + h / 2.0, &widget.value, 9.0);
                }
                if widget.data.selected {
                    ctx.set_source_rgba(0.1, 0.5, 1.0, 1.0);
                    ctx.set_line_width(2.0);
                    ctx.new_path();
                    ctx.arc(x + w / 2.0, y + h / 2.0, 13.0, 0.0, 2.0 * PI);
                    let _ = ctx.stroke();
                }
            }
            "button" | "pill_small" | "primary" | "pill" => {
                let primary = widget.kind == "primary";
                let bg: Color = if primary {
                    (0.11, 0.48, 0.96, 1.0)
                } else if dark {
                    (1.0, 1.0, 1.0, 0.09)
                } else {
                    (0.0, 0.0, 0.0, 0.06)
                };
                let radius = (15.0_f64).min(h / 2.0);
                rounded_rect(ctx, x, y, w, h, radius);
                ctx.set_source_rgba(bg.0, bg.1, bg.2, bg.3);
                let _ = ctx.fill();
                text(ctx, x + w / 2.0, y + h / 2.0 + 4.0, &widget.label, 12.0, if primary { WHITE } else { fg }, false, "center");
            }
            "switch" => {
                let on = widget.value == "true";
                rounded_rect(ctx, x, y, w, h, h / 2.0);
                if on { ctx.set_source_rgba(0.29, 0.85, 0.42, 1.0); } else { ctx.set_source_rgba(0.5, 0.5, 0.52, 0.6); }
                let _ = ctx.fill();
                let kx = x + (w - h + 6.0) * if on { 1.0 } else { 0.0 };
                circle(ctx, kx + (h - 6.0) / 2.0, y + h / 2.0, (h - 8.0) / 2.0);
                ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                let _ = ctx.fill();
            }
            "slider" => {
                let value = widget.value.parse::<f64>().unwrap_or(0.13);
                let ratio = (value - 0.08) / (0.20 - 0.08);
                ctx.set_line_width(4.0);
                let track: Color = if dark { (0.5, 0.5, 0.52, 0.6) } else { (0.0, 0.0, 0.0, 0.18) };
                ctx.set_source_rgba(track.0, track.1, track.2, track.3);
                ctx.move_to(x, y + h / 2.0);
                ctx.line_to(x + w, y + h / 2.0);
                let _ = ctx.stroke();
                ctx.set_source_rgba(0.1, 0.48, 0.96, 1.0);
                ctx.move_to(x, y + h / 2.0);
                ctx.line_to(x + w * ratio, y + h / 2.0);
                let _ = ctx.stroke();
                circle(ctx, x + w * ratio, y + h / 2.0, 8.0);
                ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                let _ = ctx.fill();
            }
            "field" => {
                rounded_rect(ctx, x, y, w, h, 7.0);
                if dark { ctx.set_source_rgba(1.0, 1.0, 1.0, 0.05); } else { ctx.set_source_rgba(0.0, 0.0, 0.0, 0.05); }
                let _ = ctx.fill();
                let label = if widget.data.recording { "Press keys".to_string() } else { widget.value.clone() };
                text(ctx, x + 8.0, y + h / 2.0 + 4.0, &label, 12.0, if label.is_empty() { secondary } else { fg }, false, "left");
            }
            "menu" => {
                rounded_rect(ctx, x, y, w, h, 7.0);
                if dark { ctx.set_source_rgba(1.0, 1.0, 1.0, 0.05); } else { ctx.set_source_rgba(0.0, 0.0, 0.0, 0.05); }
                let _ = ctx.fill();
                text(ctx, x + 10.0, y + h / 2.0 + 4.0, &widget.label, 12.0, fg, false, "left");
                icon(ctx, "chevron_left", x + w - 14.0, y + h / 2.0, 10.0, secondary);
                let _ = ctx.save();
                ctx.translate(x + w - 14.0, y + h / 2.0);
                ctx.rotate(PI);
                icon(ctx, "chevron_left", 0.0, 0.0, 10.0, secondary);
                let _ = ctx.restore();
            }
            "tab" => {
                rounded_rect(ctx, x, y, w, h, 8.0);
                ctx.set_source_rgba(1.0, 1.0, 1.0, if widget.data.selected { 0.10 } else { 0.0 });
                let _ = ctx.fill();
                rounded_rect(ctx, x + 8.0, y + 7.0, 24.0, 24.0, 6.0);
                let c = widget.data.colour;
                ctx.set_source_rgba(c.0, c.1, c.2, 0.85);
                let _ = ctx.fill();
                icon(ctx, widget.data.icon, x + 20.0, y + 19.0, 13.0, WHITE);
                text(ctx, x + 40.0, y + 23.0, &widget.label, 13.0, fg, widget.data.selected, "left");
            }
            "toolbar_slot" => {
                rounded_rect(ctx, x + 4.0, y + 2.0, w - 8.0, h - 4.0, 10.0);
                ctx.set_source_rgba(0.12, 0.12, 0.14, 0.95);
                let _ = ctx.fill();
                if let Some(thumb) = renderer.thumbnail_for(&widget.value, 40) {
                    let _ = ctx.set_source_surface(&thumb, x + (w - 40.0) / 2.0, y + 5.0);
                    let _ = ctx.paint();
                } else {
                    laser_icon(ctx, x + w / 2.0, y + h / 2.0, 18.0);
                }
                text(ctx, x + w / 2.0, y + h - 4.0, &widget.label, 9.0, SECONDARY, false, "center");
            }
            "add_slot" | "empty_slot" => {
                rounded_rect(ctx, x + 4.0, y + 2.0, w - 8.0, h - 4.0, 10.0);
                let alpha = if widget.kind == "add_slot" { 0.05 } else { 0.025 };
                ctx.set_source_rgba(0.0, 0.0, 0.0, alpha);
                let _ = ctx.fill();
                if widget.kind == "add_slot" {
                    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.16);
                    ctx.set_line_width(1.0);
                    ctx.set_dash(&[4.0, 3.0], 0.0);
                    let _ = ctx.stroke_preserve();
                    ctx.set_dash(&[], 0.0);
                    icon(ctx, "plus", x + w / 2.0, y + h / 2.0 - 5.0, 16.0, (0.4, 0.4, 0.44, 1.0));
                }
            }
            "picker_cell" => {
                rounded_rect(ctx, x, y, w, h, 9.0);
                ctx.set_source_rgba(1.0, 1.0, 1.0, if widget.data.selected { 0.08 } else { 0.0 });
                let _ = ctx.fill();
                if let Some(thumb) = renderer.thumbnail_for(&widget.value, 42) {
                    let _ = ctx.set_source_surface(&thumb, x + 11.0, y + 2.0);
                    let _ = ctx.paint();
                } else {
                    laser_icon(ctx, x + w / 2.0, y + 22.0, 18.0);
                }
                text(ctx, x + w / 2.0, y + h - 6.0, &widget.label, 8.0, secondary, false, "center");
            }
            "sound_tile" => {
                rounded_rect(ctx, x, y, w, h, 12.0);
                if widget.data.selected {
                    ctx.set_source_rgba(0.11, 0.48, 0.96, 0.14);
                } else {
                    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.035);
                }
                let _ = ctx.fill();
                let glyph = if widget.value == "off" { "speaker_slash" } else { "waveform" };
                let colour = if widget.data.selected { (0.1, 0.48, 0.96, 1.0) } else { (0.42, 0.42, 0.46, 1.0) };
                icon(ctx, glyph, x + w / 2.0, y + 24.0, 18.0, colour);
                text(ctx, x + w / 2.0, y + h - 12.0, &widget.label, 12.0, fg, widget.data.selected, "center");
            }
            "link" => {
                text(ctx, x + 14.0, y + 27.0, &widget.label, 13.0, fg, false, "left");
                icon(ctx, "arrow", x + w - 24.0, y + 22.0, 11.0, secondary);
            }
            "step_number" => {
                circle(ctx, x + w / 2.0, y + h / 2.0, w / 2.0);
                ctx.set_source_rgba(0.0, 0.0, 0.0, 0.5);
                let _ = ctx.fill();
                text(ctx, x + w / 2.0, y + h / 2.0 + 4.0, &widget.label, 11.0, WHITE, true, "center");
            }
            _ => {}
        }
        let _ = prefs;
    }
}
