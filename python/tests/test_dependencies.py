import unittest
from orbit_engine.dependencies import get_full_dependency_status


class TestDependencies(unittest.TestCase):
    def test_dependency_detection(self):
        status = get_full_dependency_status()
        self.assertIn("python_packages", status)
        self.assertIn("external_tools", status)
        self.assertIn("hardware", status)

        # Check installed modules
        pkgs = status["python_packages"]
        self.assertTrue(pkgs["rembg"]["installed"])
        self.assertTrue(pkgs["onnxruntime"]["installed"])
        self.assertTrue(pkgs["pillow"]["installed"])


if __name__ == "__main__":
    unittest.main()
