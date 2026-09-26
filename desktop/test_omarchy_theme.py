import tempfile
import unittest
from pathlib import Path

from omarchy_theme import read_theme


class OmarchyThemeTest(unittest.TestCase):
    def test_palette_and_legacy_aliases(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "colors.toml"
            path.write_text('mode = "dark"\nbg = "#123456"\nfg = "#fefefe"\naccent = "#abcdef"\n')
            mode, css = read_theme(path)
            self.assertEqual(mode, "dark")
            self.assertIn("--bg:#123456!important", css)
            self.assertIn("--tile:#abcdef!important", css)
            self.assertIn("--on-tile:#171717!important", css)

    def test_invalid_css_cannot_be_injected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "colors.toml"
            path.write_text('mode = "light"\naccent = "red; color: orange"\n')
            mode, css = read_theme(path)
            self.assertEqual(mode, "light")
            self.assertNotIn("orange", css)

    def test_dim_palette_colours_stay_readable(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "colors.toml"
            path.write_text(
                'mode = "dark"\nbackground = "#101315"\nforeground = "#cacccc"\n'
                'muted = "#4b4e55"\nred = "#565d60"\nbright_red = "#de6145"\naccent = "#798186"\n'
            )
            _, css = read_theme(path)
            self.assertNotIn("--text-muted:#4b4e55", css)
            self.assertIn("--border:#4b4e55!important", css)
            self.assertIn("--error:#de6145!important", css)
            self.assertIn("--butter:#798186!important", css)  # selection is too dark on the nav
            self.assertIn(".tile-surface {--tile-base:#798186!important;}", css)


if __name__ == "__main__":
    unittest.main()
