import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import linux_packages as PACKAGES


class LinuxPackageTests(unittest.TestCase):
    def test_package_channels_do_not_claim_each_others_commands_or_user_installations(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            payload = root / "payload"
            payload.mkdir()
            for name in ("muxy-app", "muxy", "muxy-server", "icon.png", "LICENSE"):
                (payload / name).write_bytes(b"package bytes")
            (payload / "manifest.json").write_text(json.dumps({"requirements": {
                "ubuntu_packages": ["python3", "libwebkit2gtk-4.1-0", "xdg-utils"]}}))
            owned = []

            def build(command, **kwargs):
                staging = Path(command[-2]).resolve()
                paths = {str(path.relative_to(staging)) for path in staging.rglob("*")
                         if path.is_file() or path.is_symlink()}
                control = (staging / "DEBIAN/control").read_text()
                self.assertIn("libc6 (>= 2.39)", control)
                self.assertIn("libwebkit2gtk-4.1-0 (>= 2.42)", control)
                self.assertNotIn("python3", control)
                self.assertFalse(any(".local" in path or "install.py" in path for path in paths))
                self.assertTrue(all(str(path.resolve()).startswith(str(staging))
                                    for path in staging.rglob("*") if path.is_symlink()))
                owned.append(paths - {"DEBIAN/control", "DEBIAN/preinst", "DEBIAN/prerm"})

            with patch.object(PACKAGES.subprocess, "run", side_effect=build):
                PACKAGES.package_deb("2.2.0", "x86_64", payload, root)
                PACKAGES.package_deb("2.2.0-beta.10", "arm64", payload, root)
            self.assertFalse(owned[0] & owned[1])
            self.assertIn("usr/bin/muxy", owned[0])
            self.assertIn("usr/bin/muxy-beta", owned[1])

    def test_untrusted_packaging_tool_is_not_executed_or_cached(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            def download(command, **kwargs):
                Path(command[-1]).write_bytes(b"untrusted")
            with patch.object(PACKAGES.subprocess, "run", side_effect=download) as run:
                with self.assertRaisesRegex(ValueError, "Checksum mismatch"):
                    PACKAGES.appimage_tools("arm64", directory)
            self.assertEqual(run.call_count, 1)
            self.assertFalse(list(directory.iterdir()))

if __name__ == "__main__":
    unittest.main()
