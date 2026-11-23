import json
import os
import stat
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from evdev import ecodes

from tinyhand_linux.app import TinyHandApp
from tinyhand_linux.cli import apply_startup_flags
from tinyhand_linux.input import InputManager
from tinyhand_linux.paths import runtime_dir
from tinyhand_linux.prefs import Settings
from tinyhand_linux.wayland import LayerCanvas


class _FakeBuffer:
    def __init__(self):
        self.dispatcher = {}

    def destroy(self):
        pass


class _FakePool:
    def create_buffer(self, *_args):
        return _FakeBuffer()


class _FakeShm:
    def create_pool(self, _memfd, _size):
        return _FakePool()


class TestInputRegression(unittest.TestCase):
    def test_release_is_never_debounced(self):
        events = []
        manager = InputManager(
            lambda button, pressed: events.append((button, pressed)),
            lambda dx, dy: None,
            lambda key, modifiers, value: None,
        )
        manager._key(ecodes.BTN_LEFT, 1)
        manager._key(ecodes.BTN_LEFT, 1)  # duplicate from a second mouse
        manager._key(ecodes.BTN_LEFT, 0)
        self.assertEqual(events, [("left", True), ("left", False)])


class TestShortcutRegression(unittest.TestCase):
    def test_recorded_and_settings_shortcuts_match(self):
        matches = TinyHandApp._matches_shortcut
        self.assertTrue(matches("h", {"ctrl", "alt"}, "Ctrl+Alt+H"))
        self.assertTrue(matches("x", {"ctrl", "alt"}, "Ctrl+Alt+X"))
        self.assertTrue(matches("s", {"ctrl", "alt"}, "Ctrl+Alt+S"))
        self.assertTrue(matches("b", {"super", "shift"}, "Shift+Cmd+B"))
        self.assertFalse(matches("h", {"ctrl"}, "Ctrl+Alt+H"))


class TestHiddenStartup(unittest.TestCase):
    def test_hidden_suppresses_welcome(self):
        app = SimpleNamespace(
            active=True,
            welcome_open=True,
            settings_open=False,
            ui=SimpleNamespace(welcome_open=True, settings_open=False),
        )
        apply_startup_flags(app, hidden=True)
        self.assertFalse(app.active)
        self.assertFalse(app.welcome_open)
        self.assertFalse(app.ui.welcome_open)


class TestConfigValidation(unittest.TestCase):
    def test_malformed_values_fall_back_to_defaults(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "config.json"
            path.write_text(
                json.dumps(
                    {
                        "pointer_size": "not-a-number",
                        "pointer_style": "does-not-exist",
                        "pinned_styles": "laser",
                        "fade_seconds": 999,
                        "sound_hands": "yes",
                        "tap_sound": 42,
                        "interact_key": "space",
                        "ink_choice": "chartreuse",
                    }
                )
            )
            settings = Settings.load(path)
        self.assertEqual(settings.pointer_size, 0.13)
        self.assertEqual(settings.pointer_style, "lavender")
        self.assertEqual(settings.pinned_styles, ["lavender", "glove", "coral", "plush", "robot", "laser"])
        self.assertEqual(settings.fade_seconds, 3)
        self.assertTrue(settings.sound_hands)
        self.assertEqual(settings.tap_sound, "thock")
        self.assertEqual(settings.interact_key, "option")
        self.assertEqual(settings.ink_choice, "match")

    def test_non_object_json_is_ignored(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "config.json"
            path.write_text("[1, 2, 3]")
            self.assertEqual(Settings.load(path).pointer_size, 0.13)


class TestBufferGeometry(unittest.TestCase):
    def test_non_page_aligned_resolution_creates_two_buffers(self):
        canvas = LayerCanvas()
        canvas.width = 1366
        canvas.height = 768
        canvas.shm = _FakeShm()
        canvas._create_buffers()
        self.assertEqual(len(canvas.buffers), 2)
        for item in canvas.buffers:
            item.surface.finish()
            item.mmap.close()


class TestRuntimeDirectory(unittest.TestCase):
    def test_configured_runtime_dir_is_private(self):
        with tempfile.TemporaryDirectory() as tmp:
            with patch.dict(os.environ, {"XDG_RUNTIME_DIR": tmp}):
                path = runtime_dir()
            self.assertEqual(path, Path(tmp))
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o700)


if __name__ == "__main__":
    unittest.main()
