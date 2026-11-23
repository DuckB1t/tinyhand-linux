import unittest
from types import SimpleNamespace

import cairo

from tinyhand_linux.draw import HandRenderer
from tinyhand_linux.prefs import Settings
from tinyhand_linux.ui import UI


class TestUI(unittest.TestCase):
    def setUp(self):
        self.prefs = Settings()
        self.renderer = HandRenderer()
        self.actions = []
        self.ui = UI(self.renderer, self.prefs, lambda wid, value: self.actions.append((wid, value)))

    def test_settings_controls_are_hittable(self):
        self.ui.settings_open = True
        app = SimpleNamespace(active=True, toolbar_visible=False)
        self.ui.layout(app, 1920, 1080)
        tab = next(w for w in self.ui.widgets if w.id == "section:general")
        hit = self.ui.hit(tab.x + tab.w / 2, tab.y + tab.h / 2)
        self.assertIs(hit, tab)

    def test_toolbar_style_button_is_hittable(self):
        app = SimpleNamespace(active=True, toolbar_visible=True)
        self.ui.layout(app, 1920, 1080)
        style = next(w for w in self.ui.widgets if w.id == "style:lavender")
        hit = self.ui.hit(style.x + style.w / 2, style.y + style.h / 2)
        self.assertIs(hit, style)

    def test_ink_button_respects_preference(self):
        app = SimpleNamespace(active=True, toolbar_visible=True)
        self.ui.layout(app, 1920, 1080)
        self.assertIsNotNone(next((w for w in self.ui.widgets if w.id == "ink"), None))

        self.prefs.show_ink_button = False
        self.ui.layout(app, 1920, 1080)
        self.assertIsNone(next((w for w in self.ui.widgets if w.id == "ink"), None))

    def test_hit_returns_none_outside_widgets(self):
        self.ui.settings_open = True
        app = SimpleNamespace(active=True, toolbar_visible=False)
        self.ui.layout(app, 1920, 1080)
        self.assertIsNone(self.ui.hit(5, 5))

    def test_offscreen_page_widgets_cannot_be_hit(self):
        self.ui.settings_open = True
        app = SimpleNamespace(active=True, toolbar_visible=False)
        self.ui.layout(app, 1920, 1080)
        # The startup switches are laid out below the visible 780x560 panel.
        self.assertIsNone(self.ui.hit(1250, 908))
        self.assertIsNone(self.ui.hit(1250, 960))
        sx, sy, sw, sh = self.ui.settings_rect
        for rx, ry, rw, rh in self.ui.input_rects:
            self.assertGreaterEqual(rx, sx)
            self.assertGreaterEqual(ry, sy)
            self.assertLessEqual(rx + rw, sx + sw)
            self.assertLessEqual(ry + rh, sy + sh)

    def test_settings_scroll_is_bounded(self):
        self.ui.settings_open = True
        self.ui.scroll_y = 100_000
        app = SimpleNamespace(active=True, toolbar_visible=False)
        self.ui.layout(app, 1920, 1080)
        self.assertLessEqual(self.ui.scroll_y, self.ui._scroll_max)
        tab = next(w for w in self.ui.widgets if w.id == "section:general")
        self.assertIs(self.ui.hit(tab.x + tab.w / 2, tab.y + tab.h / 2), tab)

    def test_picker_scroll_is_bounded(self):
        self.ui.settings_open = True
        self.ui.picker_open = True
        self.ui.picker_scroll = 100_000
        app = SimpleNamespace(active=True, toolbar_visible=False)
        self.ui.layout(app, 1920, 1080)
        self.assertLess(self.ui.picker_scroll, 200)
        px, py, pw, ph = self.ui.picker_rect
        cell = next(
            w
            for w in self.ui.widgets
            if w.id.startswith("picker_style:") and w.y >= py and w.y + w.h <= py + ph - 44
        )
        self.assertIs(self.ui.hit(cell.x + cell.w / 2, cell.y + cell.h / 2), cell)
        # A partially clipped cell must not be clickable above the popover.
        clipped = next(
            w for w in self.ui.widgets if w.id.startswith("picker_style:") and w.y < py
        )
        self.assertIsNot(self.ui.hit(clipped.x + clipped.w / 2, clipped.y + clipped.h / 2), clipped)

    def test_paint_all_panels(self):
        app = SimpleNamespace(active=True, toolbar_visible=True)
        surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, 1920, 1080)
        ctx = cairo.Context(surface)

        self.ui.settings_open = True
        self.ui.layout(app, 1920, 1080)
        self.ui.paint(ctx, app, 1920, 1080)

        self.ui.settings_open = False
        self.ui.welcome_open = True
        self.ui.layout(app, 1920, 1080)
        self.ui.paint(ctx, app, 1920, 1080)

        self.ui.welcome_open = False
        self.ui.ink_open = True
        self.ui.hint_parts = [("Click", "to tap")]
        self.ui.layout(app, 1920, 1080)
        self.ui.paint(ctx, app, 1920, 1080)
        surface.flush()


if __name__ == "__main__":
    unittest.main()
