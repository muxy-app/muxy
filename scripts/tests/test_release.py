import importlib.util
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("release", ROOT / "scripts/release.py")
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class BetaVersionTests(unittest.TestCase):
    def test_shallow_history_is_rejected(self):
        with patch.object(release, "git", return_value="true"):
            with self.assertRaisesRegex(ValueError, "full checkout"):
                release.checkout_version(ROOT)

    def test_version_is_beta_version_file_and_commit_count(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            git_output = {("rev-parse", "--is-shallow-repository"): "false", ("rev-list", "--count", "HEAD"): "1234"}
            with patch.object(release, "git", side_effect=lambda _, *args: git_output[args]):
                (root / "BETA_VERSION").write_text("2.1.0\n")
                self.assertEqual(release.checkout_version(root), "2.1.0-beta.1234")
                for invalid in ("", "2.1", "v2.1.0", "2.1.0-beta"):
                    (root / "BETA_VERSION").write_text(invalid)
                    with self.subTest(base=invalid), self.assertRaises(ValueError):
                        release.checkout_version(root)

    def test_release_versions_reject_other_channels_and_unsafe_paths(self):
        for version in (
            "2.0.0", "2.0.0-beta.0", "2.0.0-beta.001", "2.0.0-beta-1", "2.0-beta.1", "2.0.0.0-beta.1",
            "2.0.0-alpha.1", "2.0.0-beta.1/../x", "2.0.0-beta.1\n", "2.0.0-beta.1-beta.2",
        ):
            with self.subTest(version=version), self.assertRaises(ValueError):
                release.build_number(version)
        self.assertEqual(release.build_number("2.0.0-beta.1000"), "1000")
        self.assertEqual(release.build_number("2.1.0-beta.7"), "7")

    def test_versions_are_stable_or_beta(self):
        self.assertEqual(release.channel("2.1.0"), "stable")
        self.assertEqual(release.channel("2.1.0-beta.7"), "beta")
        for version in ("2.0.0-beta-0", "2.1", "v2.1.0", "2.1.0-rc.1", "2.1.0\n"):
            with self.subTest(version=version), self.assertRaises(ValueError):
                release.channel(version)

    def test_each_channel_installs_as_its_own_app(self):
        keys = ("CFBundleName", "CFBundleIdentifier", "CFBundleShortVersionString", "CFBundleVersion")
        info = release.bundle_info("2.1.0-beta.1234", "1234")
        self.assertEqual([info[key] for key in keys], ["Muxy Beta", "com.muxy-beta.app", "2.1.0", "1234"])
        info = release.bundle_info("2.1.0", "1234")
        self.assertEqual([info[key] for key in keys], ["Muxy", "com.muxy.app", "2.1.0", "1234"])
        for version, build in (("2.1.0-beta.1234", "1235"), ("2.1.0", "0"), ("2.1.0", "x")):
            with self.subTest(version=version, build=build), self.assertRaises(ValueError):
                release.bundle_info(version, build)


class PromotionTests(unittest.TestCase):
    def promote(self, tag="v2.0.0-beta.1234", version="2.0.0", next_beta="2.1.0", count="1234"):
        output = {("rev-parse", "--is-shallow-repository"): "false",
                  ("rev-list", "--count", f"refs/tags/{tag}"): count}
        with patch.object(release, "git", side_effect=lambda _, *args: output[args]):
            return release.promotion(ROOT, tag, version, next_beta)

    def test_a_stable_release_keeps_the_build_number_of_its_beta(self):
        self.assertEqual(self.promote(), "1234")

    def test_promotion_rejects_inputs_that_cannot_be_released(self):
        for inputs in (
            {"tag": "2.0.0-beta.1234"},
            {"tag": "v2.0.0-beta-1234"},
            {"tag": "v2.0.0"},
            {"version": "2.0.0-beta.1234"},
            {"next_beta": "2.1"},
            {"next_beta": "2.0.0"},
            {"next_beta": "1.9.0"},
            {"count": "1233"},
        ):
            with self.subTest(**inputs), self.assertRaises(ValueError):
                self.promote(**inputs)


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
        release.stamp_version(self.root, "2.0.0-beta.1234")
        release.stamp_version(self.root, "2.0.0-beta.1234")
        for name, original in before.items():
            self.assertEqual(
                (self.root / name).read_text(),
                original.replace('version = "2.0.0-beta-0"', 'version = "2.0.0-beta.1234"'),
            )


if __name__ == "__main__":
    unittest.main()
