"""Hand-drawn AppKit-style UI for TinyHand Linux.

The original macOS app uses small borderless NSPanels plus a SwiftUI settings
window.  On Wayland we paint the same panels into the layer-shell surface and
hit-test them from evdev events, which keeps the look and the interaction model
without depending on a toolkit that understands layer shells.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import Any, Callable

import cairo

from .draw import (
    icon,
    ink_dot,
    measure_text,
    panel,
    rainbow_dot,
    rounded_rect,
    text,
)
from .prefs import (
    DEFAULT_PINNED,
    INK_CHOICES,
    SOUNDS,
    STYLES,
    STYLE_BY_ID,
)

WHITE = (1, 1, 1, 1)
SECONDARY = (1, 1, 1, 0.62)
TERTIARY = (1, 1, 1, 0.38)
RED = (1, 0.26, 0.24, 1)
ORANGE = (1, 0.58, 0.12, 1)
BLUE = (0.15, 0.52, 0.98, 1)
PURPLE = (0.55, 0.42, 0.95, 1)


@dataclass
class Widget:
    id: str
    kind: str
    rect: tuple[float, float, float, float]
    label: str = ""
    value: Any = None
    data: dict[str, Any] = field(default_factory=dict)

    @property
    def x(self) -> float:
        return self.rect[0]

    @property
    def y(self) -> float:
        return self.rect[1]

    @property
    def w(self) -> float:
        return self.rect[2]

    @property
    def h(self) -> float:
        return self.rect[3]


class UI:
    def __init__(self, renderer, prefs, on_action: Callable[[str, Any], None]) -> None:
        self.renderer = renderer
        self.prefs = prefs
        self.on_action = on_action
        self.widgets: list[Widget] = []
        self.input_rects: list[tuple[int, int, int, int]] = []
        self.clip_rect: tuple[float, float, float, float] | None = None
        self.fields: dict[str, str] = {
            "idea": "",
            "hand_name": "",
        }
        self.active_field: str | None = None
        self.recording: str | None = None
        self.scroll_y = 0.0
        self.picker_open = False
        self.picker_scroll = 0.0
        self.hint_parts: list[tuple[str, str]] = []
        self.ink_open = False
        self.settings_open = False
        self.welcome_open = False
        self.settings_section = "general"
        self.toolbar_rect: tuple[float, float, float, float] = (0, 0, 0, 0)
        self.tray_rect: tuple[float, float, float, float] = (0, 0, 0, 0)
        self.hint_rect: tuple[float, float, float, float] = (0, 0, 0, 0)
        self.settings_rect: tuple[float, float, float, float] = (0, 0, 0, 0)
        self.welcome_rect: tuple[float, float, float, float] = (0, 0, 0, 0)
        self.picker_rect: tuple[float, float, float, float] = (0, 0, 0, 0)

    # -------------------------------------------------------------- geometry

    def reset(self) -> None:
        self.widgets.clear()
        self.input_rects.clear()
        self.clip_rect = None

    def add(self, widget_id: str, kind: str, rect, label="", value=None, data=None) -> Widget:
        widget = Widget(widget_id, kind, tuple(rect), label, value, data or {})
        self.widgets.append(widget)
        if kind not in ("divider", "text", "card", "keycap", "icon"):
            self.input_rects.append(
                (int(rect[0]), int(rect[1]), int(max(1, rect[2])), int(max(1, rect[3])))
            )
        return widget

    def hit(self, x: float, y: float) -> Widget | None:
        for widget in reversed(self.widgets):
            wx, wy, ww, wh = widget.rect
            if wx <= x <= wx + ww and wy <= y <= wy + wh:
                return widget
        return None

    def scroll(self, dx: int, dy: int) -> None:
        if self.picker_open:
            self.picker_scroll = max(0, self.picker_scroll + dy * 40)
        elif self.settings_open:
            self.scroll_y = max(0, self.scroll_y + dy * 40)
        else:
            return
        self.on_action("redraw", None)

    # ---------------------------------------------------------------- layout

    def layout(self, app, width: int, height: int) -> None:
        self.reset()
        if app.active and app.toolbar_visible:
            self._layout_toolbar(app, width, height)
        if self.settings_open:
            self._layout_settings(app, width, height)
        if self.welcome_open and not self.settings_open:
            self._layout_welcome(app, width, height)
        if self.picker_open and self.settings_open:
            self._layout_picker(app, width, height)

    def _layout_toolbar(self, app, width: int, height: int) -> None:
        styles = list(self.prefs.pinned_styles)
        if self.prefs.pointer_style not in styles:
            styles.append(self.prefs.pointer_style)
        count = len(styles) + 4
        panel_w = 20 + count * 40 + (count - 1) * 6
        panel_h = 56
        x = (width - panel_w) / 2
        y = height - 70
        self.toolbar_rect = (x, y, panel_w, panel_h)
        self.input_rects.append((int(x), int(y), int(panel_w), int(panel_h)))
        cx = x + 10
        for style_id in styles:
            style = STYLE_BY_ID.get(style_id)
            if not style:
                continue
            self.add(
                f"style:{style_id}",
                "style",
                (cx, y + 8, 40, 40),
                style.title,
                style_id,
                {"selected": style_id == self.prefs.pointer_style},
            )
            cx += 46
        self.add("divider", "divider", (cx, y + 16, 1, 24))
        cx += 7
        self.add(
            "ink",
            "ink_button",
            (cx, y + 8, 40, 40),
            "Ink colour",
            self.prefs.ink_choice,
            {"open": self.ink_open},
        )
        cx += 46
        self.add(
            "sound",
            "icon_button",
            (cx, y + 8, 40, 40),
            "Tap sound",
            "speaker" if self.prefs.sound_hands else "speaker_slash",
        )
        cx += 46
        self.add("exit", "icon_button", (cx, y + 8, 40, 40), "Exit  (Esc)", "x")

        if self.ink_open:
            swatches = len(INK_CHOICES)
            tray_w = 12 + swatches * 30 + (swatches - 1) * 2
            tray_h = 40
            ink_x = x + 10 + len(styles) * 46 + 7 + 20 - tray_w / 2
            tray_y = y - 8 - tray_h
            self.tray_rect = (ink_x, tray_y, tray_w, tray_h)
            self.input_rects.append((int(ink_x), int(tray_y), int(tray_w), int(tray_h)))
            sx = ink_x + 6
            for choice_id, title, colour in INK_CHOICES:
                self.add(
                    f"ink_choice:{choice_id}",
                    "swatch",
                    (sx, tray_y + 5, 30, 30),
                    title,
                    colour,
                    {"choice": choice_id, "selected": choice_id == self.prefs.ink_choice},
                )
                sx += 32

        if self.hint_parts:
            hint_w = max(260, min(760, 60 + sum(len(k + c) * 8 for k, c in self.hint_parts)))
            hint_h = 36
            hint_x = x + panel_w / 2 - hint_w / 2
            hint_y = y - 12 - hint_h
            self.hint_rect = (hint_x, hint_y, hint_w, hint_h)
            self.input_rects.append((int(hint_x), int(hint_y), int(hint_w), int(hint_h)))

    # ------------------------------------------------------------ settings UI

    def _layout_settings(self, app, width: int, height: int) -> None:
        sw, sh = 780, 560
        sx = (width - sw) / 2
        sy = (height - sh) / 2
        self.settings_rect = (sx, sy, sw, sh)
        self.input_rects.append((int(sx), int(sy), sw, sh))
        sidebar_titles = [
            ("general", "General", "gear", WHITE),
            ("pointers", "Pointers", "hand", PURPLE),
            ("appearance", "Appearance", "sparkles", (0.95, 0.45, 0.6, 1)),
            ("drawing", "Drawing", "pencil", (0.96, 0.58, 0.2, 1)),
            ("sound", "Sound", "speaker", BLUE),
            ("about", "About", "info", WHITE),
        ]
        for index, (section, title, glyph, colour) in enumerate(sidebar_titles):
            rect = (sx + 10, sy + 52 + index * 44, 190, 38)
            self.add(
                f"section:{section}",
                "tab",
                rect,
                title,
                section,
                {"icon": glyph, "colour": colour, "selected": section == self.settings_section},
            )
        self.add("settings_close", "icon_button", (sx + sw - 36, sy + 12, 24, 24), "Close", "x")

        page_x, page_y = sx + 210, sy + 48
        page_w, page_h = sw - 210, sh - 48
        self.clip_rect = (page_x, page_y, page_w, page_h)

        def page_add(wid, kind, rect, label="", value=None, data=None):
            ry = rect[1] - self.scroll_y
            if ry + rect[3] < page_y or ry > page_y + page_h:
                return None
            return self.add(wid, kind, (rect[0], ry, rect[2], rect[3]), label, value, data)

        title_x = page_x + 28
        title_y = page_y - self.scroll_y
        is_sub = self.settings_section == "make_hand"
        section_title = (
            "Make your own hand"
            if is_sub
            else next((label for key, label, _, _ in sidebar_titles if key == self.settings_section), "General")
        )
        if is_sub:
            page_add("back_pointers", "back", (title_x, title_y, 120, 24), "Pointers")
            title_y += 30
        self.add(
            "page_title",
            "title",
            (title_x, title_y, page_w - 56, 30),
            section_title,
            data={"area": "page"},
        )
        y = title_y + 46

        if self.settings_section == "general":
            y = self._general_page(page_add, title_x, y, page_w - 56)
        elif self.settings_section == "pointers":
            y = self._pointers_page(page_add, title_x, y, page_w - 56)
        elif self.settings_section == "make_hand":
            y = self._make_hand_page(page_add, title_x, y, page_w - 56)
        elif self.settings_section == "appearance":
            y = self._appearance_page(page_add, title_x, y, page_w - 56)
        elif self.settings_section == "drawing":
            y = self._drawing_page(page_add, title_x, y, page_w - 56)
        elif self.settings_section == "sound":
            y = self._sound_page(page_add, title_x, y, page_w - 56)
        elif self.settings_section == "about":
            y = self._about_page(page_add, title_x, y, page_w - 56)

        # Scrolling is driven by the settings-section state; a full-page
        # hit-test widget would sit above every control and swallow clicks.
        self._scroll_max = max(0, y + self.scroll_y - (page_y + page_h) + 60)

    def _card(self, add, x, y, w, title=None, rows=()):
        if title:
            add("card_title:" + title, "text", (x + 4, y, w - 8, 18), title, data={"area": "page", "secondary": True})
            y += 22
        height = sum(row[3] for row in rows) + 10
        self.add("card_bg", "card", (x, y, w, height), data={"area": "page"})
        for row in rows:
            y = row[0](add, x, y, w, row[1], row[2], row[3])
        return y + 18

    def _row(self, add, x, y, w, title, detail, height=52, trailing=None):
        self.add("row:" + title, "text", (x + 14, y + 8, w - 150, 18), title, data={"area": "page"})
        if detail:
            self.add("detail:" + title, "text", (x + 14, y + 26, w - 150, 28), detail, data={"area": "page", "secondary": True})
        if trailing:
            trailing(x + w - 132, y + (height - 30) / 2, 120, 30)
        return y + height

    def _switch(self, wid, value, x, y, w, h, label=""):
        self.add(wid, "switch", (x, y, w, h), label, value, {"area": "page"})

    def _pill(self, wid, label, x, y, w, h, action=None):
        self.add(wid, "pill", (x, y, w, h), label, action, {"area": "page"})

    def _general_page(self, add, x, y, w):
        self.add("session_title", "text", (x + 4, y, w, 18), "Session", data={"area": "page", "secondary": True})
        self.add("session_card", "card", (x, y + 22, w, 52), data={"area": "page"})
        self.add("session_text", "text", (x + 14, y + 32, w - 28, 18), "Wayland layer shell", data={"area": "page"})
        self.add("session_detail", "text", (x + 14, y + 50, w - 28, 18), "Hyprland / wlroots detected. No macOS accessibility permission is needed.", data={"area": "page", "secondary": True})
        y += 96

        self.add("shortcuts_title", "text", (x + 4, y, w, 18), "Shortcuts", data={"area": "page", "secondary": True})
        self.add("shortcuts_card", "card", (x, y + 22, w, 106), data={"area": "page"})
        self.add("toggle_label", "text", (x + 14, y + 34, w - 160, 18), "Turn the pointer on or off", data={"area": "page"})
        self.add("toggle_clear", "text", (x + 14, y + 86, w - 160, 18), "Clear the drawing", data={"area": "page"})
        for wid, value, ty in (
            ("field:toggle_shortcut", self.prefs.toggle_shortcut, y + 30),
            ("field:clear_shortcut", self.prefs.clear_shortcut, y + 82),
        ):
            self.add(wid, "field", (x + w - 142, ty, 128, 30), value, value, {"area": "page", "field": True, "recording": self.recording == wid.split(":", 1)[1]})
        y += 150

        self.add("clicking_title", "text", (x + 4, y, w, 18), "Clicking for real", data={"area": "page", "secondary": True})
        self.add("clicking_card", "card", (x, y + 22, w, 204), data={"area": "page"})
        rows = [
            ("Hold to use the app", "Clicks, drags and scrolls reach the app.", "interact_key"),
            ("Double-tap Shift to click through", "Clicks reach the app and the hand stays.", "double_shift"),
        ]
        for index, (title, detail, key) in enumerate(rows):
            ry = y + 32 + index * 62
            self.add(f"row:{title}", "text", (x + 14, ry, w - 160, 18), title, data={"area": "page"})
            self.add(f"detail:{title}", "text", (x + 14, ry + 18, w - 190, 30), detail, data={"area": "page", "secondary": True})
            if key == "interact_key":
                self.add("interact_key", "menu", (x + w - 142, ry + 2, 128, 30), self.prefs.interact_key, key, {"area": "page"})
            else:
                self._switch("double_shift", self.prefs.double_shift_click_through, x + w - 72, ry + 2, 52, 30)
        ry = y + 156
        self.add("while_holding", "text", (x + 14, ry, w - 160, 18), "While holding", data={"area": "page"})
        self.add("keeps_hand", "menu", (x + w - 142, ry - 2, 128, 30), "Keep the hand" if self.prefs.interact_keeps_hand else "Show the cursor", "interact_keeps_hand", {"area": "page"})
        y += 252

        self.add("startup_title", "text", (x + 4, y, w, 18), "Startup", data={"area": "page", "secondary": True})
        self.add("startup_card", "card", (x, y + 22, w, 106), data={"area": "page"})
        self._row(add, x, y + 30, w, "Open TinyHand when you log in", "Writes a standard XDG autostart entry (off by default).", 52, lambda bx, by, bw, bh: self._switch("autostart", bool(getattr(self, "autostart", False)), bx, by, bw, bh))
        self._row(add, x, y + 82, w, "Show a welcome card on first launch", "Explains the overlay once.", 52, lambda bx, by, bw, bh: self._switch("welcome_shown", self.prefs.welcome_shown, bx, by, bw, bh))
        return y + 150

    def _pointers_page(self, add, x, y, w):
        self.add("toolbar_header", "text", (x, y, 180, 18), "In your toolbar", data={"area": "page", "secondary": True})
        self.add("reset_toolbar", "pill_small", (x + w - 80, y - 4, 80, 26), "Reset", data={"area": "page"})
        y += 30
        slot_w = (w - 20 + 6) / 8
        self.add("pinned_bg", "card", (x, y, w, 58), data={"area": "page"})
        for index in range(8):
            rect = (x + 10 + index * slot_w, y + 4, slot_w - 6, 50)
            if index < len(self.prefs.pinned_styles):
                style_id = self.prefs.pinned_styles[index]
                self.add(f"pinned:{style_id}", "toolbar_slot", rect, STYLE_BY_ID.get(style_id, STYLE_BY_ID["laser"]).title, style_id, {"area": "page"})
            elif index == len(self.prefs.pinned_styles):
                self.add("add_pointer", "add_slot", rect, "Add a pointer", data={"area": "page"})
            else:
                self.add(f"empty_slot:{index}", "empty_slot", rect, data={"area": "page"})
        y += 70
        full = len(self.prefs.pinned_styles) >= 8
        helper = "The toolbar is full. Remove a pointer to add another. Drag to reorder." if full else "Press + to add a pointer. Drag to reorder, or out to remove."
        self.add("pinned_helper", "text", (x, y, w, 18), helper, data={"area": "page", "secondary": True})
        y += 34

        self.add("show_toolbar_title", "text", (x + 4, y, w, 18), "Toolbar", data={"area": "page", "secondary": True})
        self.add("show_toolbar_card", "card", (x, y + 22, w, 106), data={"area": "page"})
        self._row(add, x, y + 30, w, "Show the toolbar", "Pops up when the pointer turns on and near the bottom of the screen.", 52, lambda bx, by, bw, bh: self._switch("show_toolbar", self.prefs.show_toolbar, bx, by, bw, bh))
        self._row(add, x, y + 82, w, "Ink button", "Change the colour while you present.", 52, lambda bx, by, bw, bh: self._switch("show_ink_button", self.prefs.show_ink_button, bx, by, bw, bh))
        y += 150

        self.add("your_hands_title", "text", (x + 4, y, w, 18), "Your hands", data={"area": "page", "secondary": True})
        self.add("make_hand", "pill_small", (x + w - 130, y - 4, 130, 26), "Make your own…", data={"area": "page"})
        self.add("your_hands_card", "card", (x, y + 22, w, 74), data={"area": "page"})
        self.add("your_hands_empty", "text", (x + 14, y + 38, w - 28, 18), "Turn any idea into a hand with an image tool, then add the PNG here.", data={"area": "page", "secondary": True})
        self.add("your_hands_hint", "text", (x + 14, y + 58, w - 28, 18), "Custom hands are stored in ~/.local/share/tinyhand-linux/hands.", data={"area": "page", "secondary": True})
        return y + 120

    def _make_hand_page(self, add, x, y, w):
        steps = [
            ("1", "Describe it", "What should your hand look like?", "field:idea", self.fields["idea"]),
            ("2", "Make the image", "Open it in an image tool, then download a transparent PNG.", None, None),
            ("3", "Add it", "Drop the PNG here, or paste a path below.", "field:hand_path", self.fields.get("hand_path", "")),
        ]
        for number, title, detail, field_id, value in steps:
            self.add(f"step:{number}", "step_number", (x, y + 2, 20, 20), number, data={"area": "page"})
            self.add(f"step_title:{number}", "text", (x + 32, y, w - 32, 18), title, data={"area": "page"})
            self.add(f"step_detail:{number}", "text", (x + 32, y + 20, w - 32, 30), detail, data={"area": "page", "secondary": True})
            y += 56
            if field_id:
                self.add(field_id, "field", (x + 32, y, min(420, w - 32), 32), value, value, {"area": "page", "field": True})
                y += 46
        self.add("copy_prompt", "pill_small", (x + 32, y, 140, 30), "Copy prompt", data={"area": "page"})
        self.add("add_hand", "primary", (x + 184, y, 120, 30), "Add hand", data={"area": "page"})
        return y + 70

    def _appearance_page(self, add, x, y, w):
        self.add("pointer_card_title", "text", (x + 4, y, w, 18), "Pointer", data={"area": "page", "secondary": True})
        self.add("pointer_card", "card", (x, y + 22, w, 152), data={"area": "page"})
        self.add("size_label", "text", (x + 14, y + 34, 80, 18), "Size", data={"area": "page"})
        self.add("size_slider", "slider", (x + 100, y + 34, 180, 30), "", self.prefs.pointer_size, {"area": "page"})
        self.add("shadow_label", "text", (x + 14, y + 92, w - 100, 18), "Shadow under the pointer", data={"area": "page"})
        self._switch("show_shadow", self.prefs.show_shadow, x + w - 72, y + 88, 52, 30)
        return y + 190

    def _drawing_page(self, add, x, y, w):
        self.add("ink_title", "text", (x + 4, y, w, 18), "Ink", data={"area": "page", "secondary": True})
        self.add("ink_card", "card", (x, y + 22, w, 112), data={"area": "page"})
        detail = "Matches each pointer." if self.prefs.ink_choice == "match" else f"All pointers draw in {dict((i, t) for i, t, _ in INK_CHOICES).get(self.prefs.ink_choice, 'match').lower()}."
        self.add("ink_choice_label", "text", (x + 14, y + 34, 160, 18), "Ink colour", data={"area": "page"})
        self.add("ink_choice_detail", "text", (x + 14, y + 52, 200, 18), detail, data={"area": "page", "secondary": True})
        sx = x + 250
        for choice_id, title, colour in INK_CHOICES:
            self.add(f"settings_ink:{choice_id}", "settings_swatch", (sx, y + 36, 26, 26), title, colour, {"area": "page", "choice": choice_id, "selected": choice_id == self.prefs.ink_choice})
            sx += 34
        self.add("fade_label", "text", (x + 14, y + 86, 120, 18), "Lines fade", data={"area": "page"})
        self.add("fade_detail", "text", (x + 14, y + 104, 220, 18), f"Clear them with {self.prefs.clear_shortcut}.", data={"area": "page", "secondary": True})
        self.add("fade_menu", "menu", (x + w - 152, y + 88, 140, 30), self._fade_title(), "fade", {"area": "page"})
        return y + 160

    def _sound_page(self, add, x, y, w):
        self.add("tap_sound_title", "text", (x + 4, y, w, 18), "Tap sound", data={"area": "page", "secondary": True})
        cols = 4
        tile_w = (w - (cols - 1) * 8) / cols
        entries = [("off", "Off")] + list(SOUNDS)
        for index, (sound_id, title) in enumerate(entries):
            cx = x + (index % cols) * (tile_w + 8)
            cy = y + 30 + (index // cols) * 76
            selected = (not self.prefs.sound_hands) if sound_id == "off" else (self.prefs.sound_hands and self.prefs.tap_sound == sound_id)
            self.add(f"sound:{sound_id}", "sound_tile", (cx, cy, tile_w, 68), title, sound_id, {"area": "page", "selected": selected})
        y = y + 30 + math.ceil(len(entries) / cols) * 76 + 10
        self.add("sound_helper", "text", (x, y, w, 18), "Click a sound to hear it. It plays each time the hand taps.", data={"area": "page", "secondary": True})
        y += 40
        self.add("laser_sound_card", "card", (x, y, w, 52), data={"area": "page"})
        self.add("laser_sound_label", "text", (x + 14, y + 10, 180, 18), "Laser dot taps too", data={"area": "page"})
        self.add("laser_sound_detail", "text", (x + 14, y + 28, 240, 18), "Off by default, for quiet pointing.", data={"area": "page", "secondary": True})
        self._switch("sound_laser", self.prefs.sound_laser, x + w - 72, y + 12, 52, 30)
        return y + 90

    def _about_page(self, add, x, y, w):
        self.add("app_icon_big", "app_icon", (x, y, 72, 72), data={"area": "page"})
        self.add("app_name", "text", (x + 88, y + 6, 200, 24), "TinyHand", data={"area": "page", "bold": True, "size": 18})
        self.add("app_version", "text", (x + 88, y + 32, 220, 18), "Version 1.1.0 (Linux)", data={"area": "page", "secondary": True})
        self.add("app_tagline", "text", (x + 88, y + 52, 320, 18), "A tiny hand for your screen. Free and open source.", data={"area": "page", "secondary": True})
        y += 96
        links = [
            ("Website", "tinyhand.app", "https://tinyhand.app"),
            ("Source code", "github.com/nilbuild/tinyhand", "https://github.com/nilbuild/tinyhand"),
            ("Made by", "@nilbuild on X", "https://x.com/nilbuild"),
            ("License", "MIT", "https://github.com/nilbuild/tinyhand/blob/main/LICENSE"),
        ]
        self.add("links_card", "card", (x, y, w, 44 * len(links) + 10), data={"area": "page"})
        for index, (title, detail, url) in enumerate(links):
            ly = y + 8 + index * 44
            self.add(f"link:{index}", "link", (x, ly, w, 44), f"{title}  ·  {detail}", url, {"area": "page"})
        y += 44 * len(links) + 34
        self.add("reset_all", "pill_small", (x + w - 140, y, 140, 30), "Reset all settings…", data={"area": "page"})
        return y + 60

    def _fade_title(self) -> str:
        return {3: "After 3 seconds", 6: "After 6 seconds", 10: "After 10 seconds", 0: "Keep until cleared"}.get(self.prefs.fade_seconds, "After 3 seconds")

    # -------------------------------------------------------------- popovers

    def _layout_picker(self, app, width: int, height: int) -> None:
        pw, ph = 400, 340
        px = self.settings_rect[0] + 210 + 100
        py = self.settings_rect[1] + 150
        self.picker_rect = (px, py, pw, ph)
        self.input_rects.append((int(px), int(py), pw, ph))
        self.add("picker_bg", "card", (px, py, pw, ph), data={"picker": True})
        cell_w, cell_h = 64, 58
        cols = 5
        start_y = py + 14 - self.picker_scroll
        for index, style in enumerate(STYLES):
            col = index % cols
            row = index // cols
            rect = (px + 12 + col * 76, start_y + row * cell_h, cell_w, cell_h)
            if rect[1] + cell_h < py or rect[1] > py + ph - 44:
                continue
            self.add(f"picker_style:{style.id}", "picker_cell", rect, style.title, style.id, {"picker": True, "selected": style.id == self.prefs.pointer_style})
        self.add("picker_footer", "text", (px + 14, py + ph - 34, pw - 28, 20), "Make your own hand…", data={"picker": True, "secondary": True})
        self.add("picker_make", "invisible", (px + 14, py + ph - 36, pw - 28, 24), data={"picker": True})

    def _layout_welcome(self, app, width: int, height: int) -> None:
        w, h = 340, 300
        x, y = (width - w) / 2, (height - h) / 2
        self.welcome_rect = (x, y, w, h)
        self.input_rects.append((int(x), int(y), w, h))
        self.add("welcome_close", "icon_button", (x + w - 28, y + 14, 18, 18), "Close", "x")
        self.add("welcome_icon", "app_icon", (x + (w - 76) / 2, y + 26, 76, 76))
        title = "You're all set" if self.prefs.welcome_shown else "Welcome to TinyHand"
        subtitle = "Press " + self.prefs.toggle_shortcut + " any time to start." if self.prefs.welcome_shown else "A tiny hand for your screen."
        self.add("welcome_title", "text", (x + 20, y + 122, w - 40, 26), title, data={"welcome": True, "size": 19, "bold": True, "center": True})
        self.add("welcome_subtitle", "text", (x + 20, y + 154, w - 40, 20), subtitle, data={"welcome": True, "secondary": True, "center": True})
        self.add("welcome_button", "primary", (x + (w - 180) / 2, y + 190, 180, 34), "Start" if self.prefs.welcome_shown else "Open Settings", data={"welcome": True})
        self.add("welcome_tip", "text", (x + 20, y + 246, w - 40, 18), "Wayland layer shell · Hyprland detected", data={"welcome": True, "secondary": True, "center": True})

    # ----------------------------------------------------------------- paint

    def paint(self, ctx: cairo.Context, app, width: int, height: int) -> None:
        for widget in self.widgets:
            if widget.data.get("area") == "page" or widget.data.get("picker") or widget.data.get("welcome"):
                continue
            self._paint_widget(ctx, widget, app)

        if self.settings_open:
            sx, sy, sw, sh = self.settings_rect
            ctx.save()
            ctx.rectangle(sx + 210, sy + 48, sw - 210, sh - 48)
            ctx.clip()
            # Page background
            ctx.set_source_rgba(0.96, 0.96, 0.97, 1)
            ctx.paint()
            for widget in self.widgets:
                if widget.data.get("area") == "page":
                    self._paint_widget(ctx, widget, app, dark=False)
            ctx.restore()
            if self.scroll_y > 0:
                self._paint_scrollbar(ctx, app)

        if self.picker_open and self.settings_open:
            for widget in self.widgets:
                if widget.data.get("picker"):
                    self._paint_widget(ctx, widget, app, dark=False)

        if self.welcome_open and not self.settings_open:
            wx, wy, ww, wh = self.welcome_rect
            rounded_rect(ctx, wx, wy, ww, wh, 20)
            ctx.set_source_rgba(0.94, 0.94, 0.95, 1)
            ctx.fill()
            ctx.set_source_rgba(0, 0, 0, 0.28)
            ctx.set_line_width(1)
            ctx.stroke()
            for widget in self.widgets:
                if widget.data.get("welcome") or widget.id.startswith("welcome_"):
                    self._paint_widget(ctx, widget, app, dark=False)

    def _paint_scrollbar(self, ctx, app) -> None:
        sx, sy, sw, sh = self.settings_rect
        track_x = sx + sw - 6
        track_y = sy + 52
        track_h = sh - 60
        max_scroll = max(1, getattr(self, "_scroll_max", 1))
        knob_h = max(28, track_h * (track_h / (track_h + max_scroll)))
        pos = track_y + (self.scroll_y / max_scroll) * (track_h - knob_h)
        rounded_rect(ctx, track_x, pos, 4, knob_h, 2)
        ctx.set_source_rgba(0, 0, 0, 0.2)
        ctx.fill()

    def _paint_widget(self, ctx: cairo.Context, w: Widget, app, dark: bool = True) -> None:
        x, y, width, height = w.rect
        if width <= 0 or height <= 0:
            return
        fg = WHITE if dark else (0.11, 0.11, 0.13, 1)
        secondary = SECONDARY if dark else (0.42, 0.42, 0.46, 1)
        card_fill = (1, 1, 1, 0.045) if dark else (0, 0, 0, 0.035)

        if w.kind == "invisible":
            return
        if w.kind == "divider":
            ctx.set_source_rgba(1, 1, 1, 0.18)
            ctx.rectangle(x, y, width, height)
            ctx.fill()
            return
        if w.kind == "card":
            rounded_rect(ctx, x, y, width, height, 12)
            ctx.set_source_rgba(*card_fill)
            ctx.fill()
            return
        if w.kind == "text":
            size = w.data.get("size", 11 if w.data.get("secondary") else 13)
            colour = secondary if w.data.get("secondary") else fg
            if w.data.get("center"):
                text(ctx, x + width / 2, y + size + 2, w.label, size, colour, w.data.get("bold", False), "center")
            else:
                text(ctx, x, y + size, w.label, size, colour, w.data.get("bold", False))
            return
        if w.kind == "title":
            text(ctx, x, y + 26, w.label, 22, fg, True)
            return
        if w.kind == "back":
            icon(ctx, "chevron_left", x + 6, y + 12, 12, secondary)
            text(ctx, x + 18, y + 16, w.label, 12, secondary)
            return
        if w.kind == "app_icon":
            self._app_icon(ctx, x, y, width, height)
            return
        if w.kind == "icon":
            icon(ctx, w.value, x + width / 2, y + height / 2, min(width, height) * 0.55, fg)
            return
        if w.kind == "icon_button":
            icon(ctx, w.value, x + width / 2, y + height / 2, min(width, height) * 0.45, (1, 1, 1, 0.85) if dark else (0.2, 0.2, 0.22, 1))
            return
        if w.kind == "style":
            self._style_button(ctx, w, dark)
            return
        if w.kind == "ink_button":
            rounded_rect(ctx, x, y, width, height, 10)
            ctx.set_source_rgba(1, 1, 1, 0.18 if w.data.get("open") else 0.0)
            ctx.fill()
            choice = next((c for i, _, c in INK_CHOICES if i == w.value), None)
            if w.value == "match":
                rainbow_dot(ctx, x + width / 2, y + height / 2)
            else:
                ink_dot(ctx, x + width / 2, y + height / 2, choice or "#FFFFFF")
            return
        if w.kind == "swatch":
            if w.value is None:
                rainbow_dot(ctx, x + width / 2, y + height / 2, 13)
            else:
                ink_dot(ctx, x + width / 2, y + height / 2, w.value, 12)
            if w.data.get("selected"):
                ctx.set_source_rgba(1, 1, 1, 0.2)
                ctx.arc(x + width / 2, y + height / 2, 15, 0, 2 * math.pi)
                ctx.stroke()
            return
        if w.kind == "settings_swatch":
            if w.value is None:
                rainbow_dot(ctx, x + width / 2, y + height / 2, 9)
            else:
                ink_dot(ctx, x + width / 2, y + height / 2, w.value, 9)
            if w.data.get("selected"):
                ctx.set_source_rgba(0.1, 0.5, 1, 1)
                ctx.set_line_width(2)
                ctx.arc(x + width / 2, y + height / 2, 13, 0, 2 * math.pi)
                ctx.stroke()
            return
        if w.kind == "button" or w.kind == "pill_small" or w.kind == "primary":
            if w.kind == "primary":
                bg = (0.11, 0.48, 0.96, 1)
            else:
                bg = (1, 1, 1, 0.09) if dark else (0, 0, 0, 0.06)
            rounded_rect(ctx, x, y, width, height, min(15, height / 2))
            ctx.set_source_rgba(*bg)
            ctx.fill()
            text(ctx, x + width / 2, y + height / 2 + 4, w.label, 12, (1, 1, 1, 1) if w.kind == "primary" else fg, False, "center")
            return
        if w.kind == "pill":
            rounded_rect(ctx, x, y, width, height, height / 2)
            ctx.set_source_rgba(*((1, 1, 1, 0.09) if dark else (0, 0, 0, 0.06)))
            ctx.fill()
            text(ctx, x + width / 2, y + height / 2 + 4, w.label, 12, fg, False, "center")
            return
        if w.kind == "switch":
            rounded_rect(ctx, x, y, width, height, height / 2)
            ctx.set_source_rgba(*((0.29, 0.85, 0.42, 1) if w.value else (0.5, 0.5, 0.52, 0.6)))
            ctx.fill()
            kx = x + (width - height + 6) * (1 if w.value else 0)
            ctx.set_source_rgba(1, 1, 1, 1)
            ctx.arc(kx + (height - 6) / 2, y + height / 2, (height - 8) / 2, 0, 2 * math.pi)
            ctx.fill()
            return
        if w.kind == "slider":
            value = float(w.value)
            minv, maxv = 0.08, 0.20
            ratio = (value - minv) / (maxv - minv)
            ctx.set_source_rgba(*((0.5, 0.5, 0.52, 0.6) if dark else (0, 0, 0, 0.18)))
            ctx.set_line_width(4)
            ctx.move_to(x, y + height / 2)
            ctx.line_to(x + width, y + height / 2)
            ctx.stroke()
            ctx.set_source_rgba(0.1, 0.48, 0.96, 1)
            ctx.move_to(x, y + height / 2)
            ctx.line_to(x + width * ratio, y + height / 2)
            ctx.stroke()
            ctx.set_source_rgba(1, 1, 1, 1)
            ctx.arc(x + width * ratio, y + height / 2, 8, 0, 2 * math.pi)
            ctx.fill()
            return
        if w.kind == "field":
            rounded_rect(ctx, x, y, width, height, 7)
            ctx.set_source_rgba(*((1, 1, 1, 0.05) if dark else (0, 0, 0, 0.05)))
            ctx.fill()
            ctx.set_source_rgba(*((1, 1, 1, 0.18) if dark else (0, 0, 0, 0.12)))
            ctx.set_line_width(1)
            ctx.stroke()
            if w.data.get("recording"):
                label = "Press keys"
            else:
                label = w.value or ""
            text(ctx, x + 8, y + height / 2 + 4, label, 12, secondary if not label else fg)
            return
        if w.kind == "menu":
            rounded_rect(ctx, x, y, width, height, 7)
            ctx.set_source_rgba(*((1, 1, 1, 0.05) if dark else (0, 0, 0, 0.05)))
            ctx.fill()
            text(ctx, x + 10, y + height / 2 + 4, w.label, 12, fg)
            icon(ctx, "chevron_left", x + width - 14, y + height / 2, 10, secondary)
            ctx.save()
            ctx.translate(x + width - 14, y + height / 2)
            ctx.rotate(math.pi)
            icon(ctx, "chevron_left", 0, 0, 10, secondary)
            ctx.restore()
            return
        if w.kind == "tab":
            selected = w.data.get("selected")
            rounded_rect(ctx, x, y, width, height, 8)
            ctx.set_source_rgba(1, 1, 1, 0.10 if selected else 0.0)
            ctx.fill()
            tile_x, tile_y = x + 8, y + 7
            rounded_rect(ctx, tile_x, tile_y, 24, 24, 6)
            colour = w.data.get("colour", WHITE)
            ctx.set_source_rgba(colour[0], colour[1], colour[2], 0.85)
            ctx.fill()
            icon(ctx, w.data.get("icon", "info"), tile_x + 12, tile_y + 12, 13, (1, 1, 1, 1))
            text(ctx, x + 40, y + 23, w.label, 13, fg, selected)
            return
        if w.kind == "toolbar_slot":
            rounded_rect(ctx, x + 4, y + 2, width - 8, height - 4, 10)
            ctx.set_source_rgba(0.12, 0.12, 0.14, 0.95)
            ctx.fill()
            thumb = self.renderer.thumbnail_for(w.value, 40)
            if thumb:
                ctx.set_source_surface(thumb, x + (width - 40) / 2, y + 5)
                ctx.paint()
            else:
                self._laser_icon(ctx, x + width / 2, y + height / 2, 18)
            text(ctx, x + width / 2, y + height - 4, w.label, 9, SECONDARY, False, "center")
            return
        if w.kind == "add_slot":
            rounded_rect(ctx, x + 4, y + 2, width - 8, height - 4, 10)
            ctx.set_source_rgba(0, 0, 0, 0.05)
            ctx.fill()
            ctx.set_source_rgba(0, 0, 0, 0.16)
            ctx.set_line_width(1)
            ctx.set_dash([4, 3])
            ctx.stroke()
            ctx.set_dash([])
            icon(ctx, "plus", x + width / 2, y + height / 2 - 5, 16, (0.4, 0.4, 0.44, 1))
            return
        if w.kind == "empty_slot":
            rounded_rect(ctx, x + 4, y + 2, width - 8, height - 4, 10)
            ctx.set_source_rgba(0, 0, 0, 0.025)
            ctx.fill()
            return
        if w.kind == "picker_cell":
            rounded_rect(ctx, x, y, width, height, 9)
            ctx.set_source_rgba(1, 1, 1, 0.08 if w.data.get("selected") else 0.0)
            ctx.fill()
            thumb = self.renderer.thumbnail_for(w.value, 42)
            if thumb:
                ctx.set_source_surface(thumb, x + 11, y + 2)
                ctx.paint()
            else:
                self._laser_icon(ctx, x + width / 2, y + 22, 18)
            text(ctx, x + width / 2, y + height - 6, w.label, 8, secondary, False, "center")
            return
        if w.kind == "sound_tile":
            selected = w.data.get("selected")
            rounded_rect(ctx, x, y, width, height, 12)
            ctx.set_source_rgba(*((0.11, 0.48, 0.96, 0.14) if selected else (0, 0, 0, 0.035)))
            ctx.fill()
            if selected:
                ctx.set_source_rgba(0.11, 0.48, 0.96, 0.7)
                ctx.set_line_width(1.5)
                ctx.stroke()
            icon_name = "speaker_slash" if w.value == "off" else "waveform"
            icon(ctx, icon_name, x + width / 2, y + 24, 18, (0.1, 0.48, 0.96, 1) if selected else (0.42, 0.42, 0.46, 1))
            text(ctx, x + width / 2, y + height - 12, w.label, 12, fg, selected, "center")
            return
        if w.kind == "link":
            text(ctx, x + 14, y + 27, w.label, 13, fg)
            icon(ctx, "arrow", x + width - 24, y + 22, 11, secondary)
            return
        if w.kind == "step_number":
            ctx.set_source_rgba(0, 0, 0, 0.5)
            ctx.arc(x + width / 2, y + height / 2, width / 2, 0, 2 * math.pi)
            ctx.fill()
            text(ctx, x + width / 2, y + height / 2 + 4, w.label, 11, (1, 1, 1, 1), True, "center")
            return

    def _style_button(self, ctx, w: Widget, dark: bool) -> None:
        x, y, width, height = w.rect
        rounded_rect(ctx, x, y, width, height, 10)
        ctx.set_source_rgba(1, 1, 1, 0.18 if w.data.get("selected") else 0.0)
        ctx.fill()
        thumb = self.renderer.thumbnail_for(w.value, 40)
        if thumb:
            ctx.set_source_surface(thumb, x, y)
            ctx.paint()
        else:
            self._laser_icon(ctx, x + width / 2, y + height / 2, 20)

    def _laser_icon(self, ctx, cx, cy, size) -> None:
        r = size / 2
        ctx.set_source_rgba(1, 0.16, 0.12, 0.35)
        ctx.arc(cx, cy, r * 1.6, 0, 2 * math.pi)
        ctx.fill()
        ctx.set_source_rgba(1, 0.16, 0.12, 1)
        ctx.arc(cx, cy, r * 0.65, 0, 2 * math.pi)
        ctx.fill()
        ctx.set_source_rgba(1, 0.86, 0.82, 1)
        ctx.arc(cx, cy, r * 0.28, 0, 2 * math.pi)
        ctx.fill()

    def _app_icon(self, ctx, x, y, width, height) -> None:
        rounded_rect(ctx, x, y, width, height, min(16, width * 0.22))
        ctx.set_source_rgba(0.10, 0.0, 0.12, 1)
        ctx.fill()
        self._laser_icon(ctx, x + width / 2, y + height / 2, width * 0.45)
