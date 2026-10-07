import importlib.util
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("beta_release", ROOT / "scripts/beta_release.py")
beta = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(beta)


class BetaVersionTests(unittest.TestCase):
    def test_shallow_history_is_rejected(self):
        with patch.object(beta, "git", return_value="true"):
            with self.assertRaisesRegex(ValueError, "full checkout"):
                beta.checkout_version(ROOT)

    def test_release_versions_reject_other_channels_and_unsafe_paths(self):
        for version in (
            "2.0.0", "2.0.0-beta-0", "2.0.0-beta-001", "2.0.0-beta.1",
            "2.0.0-alpha-1", "2.1.0-beta-1", "2.0.0-beta-1/../x", "2.0.0-beta-1\n",
        ):
            with self.subTest(version=version), self.assertRaises(ValueError):
                beta.build_number(version)
        self.assertEqual(beta.build_number("2.0.0-beta-1000"), "1000")


class StampTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for source in [ROOT / "Cargo.toml", ROOT / "Cargo.lock", *ROOT.glob("crates/*/Cargo.toml")]:
            destination = self.root / source.relative_to(ROOT)
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)

    def test_stamp_changes_only_workspace_versions_and_is_repeatable(self):
        before = {name: (self.root / name).read_text() for name in ("Cargo.toml", "Cargo.lock")}
        beta.stamp_version(self.root, "2.0.0-beta-1234")
        beta.stamp_version(self.root, "2.0.0-beta-1234")
        for name, original in before.items():
            self.assertEqual(
                (self.root / name).read_text(),
                original.replace('version = "2.0.0-beta-0"', 'version = "2.0.0-beta-1234"'),
            )


if __name__ == "__main__":
    unittest.main()
