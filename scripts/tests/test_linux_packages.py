import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from concurrent.futures import ThreadPoolExecutor
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import linux_packages as PACKAGES


class LinuxPackageTests(unittest.TestCase):
    def test_beta_debian_versions_sort_before_stable_and_keep_numeric_build_order(self):
        if not shutil.which("dpkg"):
            self.skipTest("Debian version comparison requires dpkg")
        versions = ["2.2.0-beta.9", "2.2.0-beta.10", "2.2.0", "2.2.1-beta.11"]
        for old, new in zip(versions, versions[1:]):
            subprocess.run(["dpkg", "--compare-versions", PACKAGES.debian_version(old),
                            "lt", PACKAGES.debian_version(new)], check=True)

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
                paths = {path.relative_to(staging).as_posix() for path in staging.rglob("*")
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

    @unittest.skipUnless(sys.platform == "linux" and os.getuid() != 0, "AppRun requires a non-root Linux user")
    def test_appimage_launches_concurrently_from_durable_paths_without_polluting_server_environment(self):
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            appdir = home / "mounted image"
            bins = appdir / "usr/bin"
            bins.mkdir(parents=True)
            shutil.copyfile(ROOT / "packaging/linux/AppRun", appdir / "AppRun")
            shutil.copyfile(ROOT / "scripts/install-desktop-linux.py", appdir / "install.py")
            (appdir / "version").write_text("2.2.0-beta.10\n")
            (appdir / "bundle-id").write_text("a" * 64 + "\n")
            (bins / "muxy-app").write_text('#!/bin/sh\nHERE=$(dirname "$0")\nexec "${MUXY_SERVER_BIN:-$HERE/muxy-server}" "$@"\n')
            (bins / "muxy-server").write_text('#!/bin/sh\nprintf "%s\\n" "$0"\nprintf "umask=%s\\n" "$(umask)"\nenv\n')
            (bins / "muxy").write_text('#!/bin/sh\nprintf "%s\\n" "$0"\nprintf "umask=%s\\n" "$(umask)"\n')
            for path in [appdir / "AppRun", *bins.iterdir()]:
                path.chmod(0o755)
            env = {key: value for key, value in os.environ.items() if not key.startswith("MUXY_")}
            env.update(HOME=str(home), XDG_DATA_HOME=str(home / "data"), WEBKIT_EXEC_PATH="user setting",
                       GIO_EXTRA_MODULES="", APPDIR=str(appdir), APPIMAGE="temporary.AppImage")
            def launch(_):
                return subprocess.check_output([str(appdir / "AppRun")], env=env, text=True, timeout=20, umask=0o027)
            with ThreadPoolExecutor(max_workers=4) as workers:
                results = list(workers.map(launch, range(4)))
            durable = home / "data/muxy-appimage" / ("a" * 64)
            custom = subprocess.check_output([str(appdir / "AppRun")], text=True, timeout=20,
                                            env={**env, "MUXY_SERVER_BIN": str(bins / "muxy-server")})
            self.assertIn(f"MUXY_SERVER_BIN={bins / 'muxy-server'}", custom.splitlines())
            self.assertIn("WEBKIT_EXEC_PATH=user setting", custom.splitlines())
            self.assertFalse(any(line.startswith("MUXY_APPIMAGE_") for line in custom.splitlines()))
            shutil.rmtree(appdir)
            for result in results:
                lines = result.splitlines()
                self.assertEqual(lines[0], str(durable / "usr/bin/muxy-server"))
                self.assertEqual(int(lines[1].removeprefix("umask="), 8), 0o027)
                self.assertIn("WEBKIT_EXEC_PATH=user setting", lines)
                self.assertIn("GIO_EXTRA_MODULES=", lines)
                self.assertFalse(any(line.startswith(("MUXY_APPIMAGE_", "WEBKIT_INJECTED_BUNDLE_PATH=",
                                                      "MUXY_SERVER_BIN=", "APPDIR=", "APPIMAGE=")) for line in lines))
            self.assertTrue((durable / "usr/bin/muxy").is_file())
            self.assertEqual(len(list((home / "data/muxy-appimage").glob(".stage-*"))), 0)
            cli = subprocess.check_output([str(durable / "AppRun"), "--cli"], env=env, text=True, umask=0o022)
            self.assertEqual(int(cli.splitlines()[1].removeprefix("umask="), 8), 0o022)

    @unittest.skipUnless(sys.platform == "linux" and os.getuid() != 0, "AppRun requires a non-root Linux user")
    def test_appimage_refuses_a_symlinked_executable_store(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "data").mkdir()
            (root / "unrelated").mkdir()
            (root / "data/muxy-appimage").symlink_to(root / "unrelated")
            result = subprocess.run(["sh", str(ROOT / "packaging/linux/AppRun")], capture_output=True,
                                    env={**os.environ, "XDG_DATA_HOME": str(root / "data")})
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"must not be a symlink", result.stderr)
            self.assertFalse(list((root / "unrelated").iterdir()))


if __name__ == "__main__":
    unittest.main()
