import json
import tempfile
import unittest
from pathlib import Path

from tinyhand_linux.paths import asset_dir
from tinyhand_linux.prefs import (
    DEFAULT_PINNED,
    SOUNDS,
    STYLES,
    RuntimeState,
    Settings,
    migrate_style,
    resolved_ink,
)


class TestCatalogue(unittest.TestCase):
    def test_style_and_sound_counts(self):
        self.assertEqual(len(STYLES), 37)
        self.assertEqual(len(SOUNDS), 15)
        self.assertEqual(len(DEFAULT_PINNED), 6)

    def test_all_assets_exist(self):
        manifest = json.loads((asset_dir() / "manifest.json").read_text())
        self.assertEqual(len(manifest), 36)
        for style in STYLES:
            if style.is_laser:
                continue
            self.assertIn(style.id, manifest)
            self.assertTrue((asset_dir() / manifest[style.id]["file"]).exists())
        for sound, _title in SOUNDS:
            self.assertTrue((asset_dir() / "sounds" / f"tap_{sound}.wav").exists())

    def test_ink_resolution(self):
        self.assertEqual(resolved_ink("match", "coral"), (1.0, 0.1607843137254902, 0.12549019607843137))
        self.assertEqual(resolved_ink("white", "coral"), (1.0, 1.0, 1.0))

    def test_migrations(self):
        self.assertEqual(migrate_style("dot"), "laser")
        self.assertEqual(migrate_style("sunny"), "coral")
        self.assertEqual(migrate_style("lavender"), "lavender")


class TestSettings(unittest.TestCase):
    def test_defaults(self):
        settings = Settings()
        self.assertEqual(settings.pointer_size, 0.13)
        self.assertEqual(settings.fade_seconds, 3)
        self.assertEqual(settings.tap_sound, "thock")
        self.assertTrue(settings.show_toolbar)
        self.assertFalse(settings.sound_laser)

    def test_round_trip(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "config.json"
            settings = Settings()
            settings.pointer_style = "robot"
            settings.pinned_styles.append("mint")
            settings.save(path)
            loaded = Settings.load(path)
            self.assertEqual(loaded.pointer_style, "robot")
            self.assertIn("mint", loaded.pinned_styles)

    def test_reset_keeps_one_pinned_style(self):
        settings = Settings()
        settings.pinned_styles = ["coral"]
        self.assertFalse(settings.unpin("coral"))
        settings.reset()
        self.assertEqual(settings.pinned_styles, list(DEFAULT_PINNED))


class TestRuntimeState(unittest.TestCase):
    def test_state_shape(self):
        state = RuntimeState.build(123, True, "laser", False, 2)
        data = json.loads(json.dumps(state.__dict__))
        self.assertEqual(data["pid"], 123)
        self.assertTrue(data["active"])
        self.assertEqual(data["style"], "laser")
        self.assertFalse(data["sound_on"])
        self.assertEqual(data["strokes"], 2)
        self.assertEqual(len(data["styles"]), 37)


if __name__ == "__main__":
    unittest.main()
