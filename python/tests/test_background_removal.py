import os
import tempfile
import unittest
from pathlib import Path
from PIL import Image, ImageDraw

from orbit_engine.background_removal import AIBackgroundRemover


class TestBackgroundRemoval(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.mkdtemp()
        self.input_img = Path(self.temp_dir) / "test_input.png"
        self.output_img = Path(self.temp_dir) / "test_output.png"

        # Create a simple test image: a red circle with black background
        img = Image.new("RGB", (128, 128), color=(0, 0, 0))
        draw = ImageDraw.Draw(img)
        draw.ellipse([(32, 32), (96, 96)], fill=(255, 0, 0))
        img.save(self.input_img, format="PNG")

    def tearDown(self):
        import shutil
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_remove_background_execution(self):
        remover = AIBackgroundRemover(default_model="u2net")
        # Test background removal
        res = remover.remove_background(
            str(self.input_img),
            str(self.output_img),
            model_name="u2net",
            alpha_matting=False,
        )

        self.assertTrue(self.output_img.exists())
        self.assertGreater(self.output_img.stat().st_size, 0)
        self.assertEqual(res["dimensions"], [128, 128])

        # Verify output is RGBA
        with Image.open(self.output_img) as out:
            self.assertEqual(out.mode, "RGBA")
            self.assertEqual(out.size, (128, 128))


if __name__ == "__main__":
    unittest.main()
