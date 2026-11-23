import unittest

import cairo

from tinyhand_linux.draw import HandRenderer, draw_stroke, ellipse, panel, rounded_rect, text


class TestDraw(unittest.TestCase):
    def test_cairo_primitives(self):
        surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, 240, 180)
        ctx = cairo.Context(surface)
        rounded_rect(ctx, 10, 10, 100, 48, 12)
        panel(ctx, 4, 70, 200, 60, radius=14, alpha=0.7)
        ellipse(ctx, 180, 100, 20, 12)
        text(ctx, 20, 160, "TinyHand", 13, (1, 1, 1, 1), True)
        surface.flush()

    def test_smoothed_stroke_with_arrowhead(self):
        surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, 300, 200)
        ctx = cairo.Context(surface)
        points = [
            (10.0, 10.0, 0.0),
            (70.0, 140.0, 0.1),
            (150.0, 40.0, 0.2),
            (280.0, 120.0, 0.3),
        ]
        draw_stroke(ctx, points, (1.0, 0.25, 0.1), width=5, arrow=True)
        draw_stroke(ctx, points[:1], (1, 1, 1), width=5)
        surface.flush()

    def test_hand_thumbnails(self):
        renderer = HandRenderer()
        thumb = renderer.thumbnail_for("lavender", 40)
        self.assertIsNotNone(thumb)
        assert thumb is not None
        self.assertEqual((thumb.get_width(), thumb.get_height()), (40, 40))
        self.assertIsNone(renderer.thumbnail_for("laser", 40))
        self.assertIsNone(renderer.thumbnail_for("not-a-style", 40))


if __name__ == "__main__":
    unittest.main()
