import importlib.util
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("homebrew", ROOT / "scripts/homebrew.py")
homebrew = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(homebrew)

ASSETS = [
    "Muxy-2.0.0-arm64.dmg", "Muxy-2.0.0-x86_64.dmg",
    "muxy-2.0.0-macos-arm64.zip", "muxy-2.0.0-macos-x86_64.zip",
    "muxy-2.0.0-linux-arm64.tar.gz", "muxy-2.0.0-linux-x86_64.tar.gz",
]


class HomebrewTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.sums = self.root / "SHA256SUMS"
        self.digests = {name: f"{index:x}" * 64 for index, name in enumerate(ASSETS)}
        self.sums.write_text("".join(f"{digest}  {name}\n" for name, digest in self.digests.items()))
        self.tap = self.root / "tap"
        self.tap.mkdir()

    def test_cask_and_formula_pin_every_platform_of_the_release(self):
        homebrew.write("2.0.0", self.sums, self.tap, "muxy-app/muxy")
        cask = (self.tap / "Casks/muxy.rb").read_text()
        formula = (self.tap / "Formula/muxy-cli.rb").read_text()
        self.assertIn('version "2.0.0"', cask)
        self.assertIn('app "Muxy.app"', cask)
        self.assertIn('"~/Library/Application Support/Muxy 2"', cask)
        self.assertIn("/releases/download/v#{version}/Muxy-#{version}-#{arch}.dmg", cask)
        for name, digest in self.digests.items():
            self.assertIn(f'"{digest}"', cask if name.endswith(".dmg") else formula)
            if not name.endswith(".dmg"):
                self.assertIn(f"/releases/download/v2.0.0/{name}", formula)
        self.assertIn('bin.install "muxy", "muxy-server"', formula)

    def test_only_complete_stable_releases_are_published(self):
        with self.assertRaisesRegex(ValueError, "stable"):
            homebrew.write("2.0.0-beta.1150", self.sums, self.tap, "muxy-app/muxy")
        self.sums.write_text("".join(line + "\n" for line in self.sums.read_text().splitlines()[:-1]))
        with self.assertRaisesRegex(ValueError, "muxy-2.0.0-linux-x86_64.tar.gz"):
            homebrew.write("2.0.0", self.sums, self.tap, "muxy-app/muxy")
        self.sums.write_text("not-a-digest  Muxy-2.0.0-arm64.dmg\n")
        with self.assertRaises(ValueError):
            homebrew.write("2.0.0", self.sums, self.tap, "muxy-app/muxy")


if __name__ == "__main__":
    unittest.main()
