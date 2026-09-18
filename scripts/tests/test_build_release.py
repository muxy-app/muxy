import json
import os
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERSION = "2.0.0-beta-1234"
COMPATIBILITY = int(re.search(r"pub const COMPATIBILITY: u[0-9]+ = ([0-9]+)", (ROOT / "crates/muxy-protocol/src/build.rs").read_text())[1])
TARGETS = {"arm64": "aarch64-apple-darwin", "x86_64": "x86_64-apple-darwin"}

FAKE_TOOL = r'''
import json, os, shutil, sys
from pathlib import Path
name = Path(sys.argv[0]).name
args = sys.argv[1:]
with open(os.environ["TOOL_LOG"], "a") as log:
    log.write(json.dumps([name, *args]) + "\n")
if name == "uname":
    print("Darwin")
elif name == "lipo":
    print(os.environ["TEST_ARCH"])
elif name == "otool":
    print(args[-1] + ":\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0)")
elif name == "ditto":
    if args[0] == "-c":
        Path(args[-1]).write_bytes(b"symbols")
    else:
        shutil.copytree(*args)
elif name == "sips":
    shutil.copyfile(args[3], args[5])
elif name == "iconutil":
    Path(args[3]).write_bytes(b"icns")
elif name == "hdiutil":
    Path(args[-1]).write_bytes(b"dmg")
elif name not in ("cargo", "strip", "plutil", "codesign"):
    sys.exit("unexpected tool: " + name)
'''


class BuildReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for relative in (
            "scripts/build-release.sh", "scripts/beta_release.py", "scripts/beta_compatibility.py", "scripts/zig/zig",
            "crates/muxy-protocol/src/build.rs", "LICENSE", "crates/muxy-server/src/detection/THIRD_PARTY.md", "crates/muxy-server/src/detection/LICENSE-herdr",
            "packaging/macos/AppIcon.png", "packaging/macos/AppIconBeta.png",
        ):
            destination = self.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, destination)
        for target in TARGETS.values():
            binaries = self.root / "target" / target / "release"
            binaries.mkdir(parents=True)
            for name in ("muxy-app", "muxy", "muxy-server"):
                (binaries / name).write_text(f"#!{sys.executable}\nimport json\nprint(json.dumps({{'version': '{VERSION}', 'compatibility': {COMPATIBILITY}}}))\n")
                (binaries / f"{name}.dSYM").mkdir()
        tools = self.root / "tools"
        tools.mkdir()
        for name in (
            "uname", "cargo", "lipo", "otool", "ditto", "strip", "plutil",
            "sips", "iconutil", "codesign", "hdiutil", "zig",
        ):
            tool = tools / name
            tool.write_text(f"#!{sys.executable}\n" + FAKE_TOOL)
            tool.chmod(0o755)
        self.log = self.root / "tools.jsonl"
        self.env = {
            **os.environ,
            "PATH": str(tools) + os.pathsep + os.environ["PATH"],
            "TOOL_LOG": str(self.log),
        }

    def build(self, arch="arm64", version=VERSION):
        return subprocess.run(
            ["bash", str(self.root / "scripts/build-release.sh"),
             "--arch", arch, "--version", version],
            env={**self.env, "TEST_ARCH": arch}, capture_output=True, text=True,
        )

    def calls(self, tool):
        if not self.log.exists():
            return []
        return [entry for line in self.log.read_text().splitlines()
                if (entry := json.loads(line))[0] == tool]

    def test_beta_build_uses_dedicated_icon_for_every_size_and_architecture(self):
        source = str(self.root / "packaging/macos/AppIconBeta.png")
        expected = {
            (str(size * factor), f"icon_{size}x{size}{suffix}.png")
            for size in (16, 32, 128, 256, 512)
            for factor, suffix in ((1, ""), (2, "@2x"))
        }
        for arch in TARGETS:
            with self.subTest(arch=arch):
                self.log.unlink(missing_ok=True)
                result = self.build(arch)
                self.assertEqual(result.returncode, 0, result.stderr)
                calls = self.calls("sips")
                self.assertEqual(len(calls), 10)
                for call in calls:
                    self.assertEqual(call[1:6], ["-z", call[2], call[2], source, "--out"])
                self.assertEqual({(call[2], Path(call[6]).name) for call in calls}, expected)
                app = self.root / "target/beta" / VERSION / arch / "Muxy Beta.app"
                info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
                icon = info["CFBundleIconFile"] + ".icns"
                self.assertTrue((app / "Contents/Resources" / icon).is_file())
                conversion = self.calls("iconutil")
                self.assertEqual(len(conversion), 1)
                self.assertEqual(conversion[0][1:4], ["--convert", "icns", "--output"])
                self.assertTrue(conversion[0][4].endswith(f"/Contents/Resources/{icon}"))

    def test_bundle_builds_and_signs_three_separate_executables(self):
        result = self.build()
        self.assertEqual(result.returncode, 0, result.stderr)
        binaries = self.root / "target/beta" / VERSION / "arm64/Muxy Beta.app/Contents/MacOS"
        canonical, alias = binaries / "muxy", binaries / "muxy-server"
        self.assertEqual(canonical.read_bytes(), alias.read_bytes())
        self.assertNotEqual(canonical.stat().st_ino, alias.stat().st_ino)
        build = next(call for call in self.calls("cargo") if call[1] == "build")
        self.assertIn("muxy-cli", build)
        self.assertIn("muxy-server", build)
        signing = [call[-1] for call in self.calls("codesign") if "--sign" in call]
        self.assertEqual(sum(path.endswith("/muxy") for path in signing), 1)
        self.assertEqual(sum(path.endswith("/muxy-server") for path in signing), 1)

    def test_standalone_zip_contains_exact_bundle_executable_bytes(self):
        result = self.build()
        self.assertEqual(result.returncode, 0, result.stderr)
        output = self.root / "target/beta" / VERSION / "arm64"
        with zipfile.ZipFile(output / f"muxy-{VERSION}-macos-arm64.zip") as archive:
            self.assertEqual(set(archive.namelist()), {"muxy", "muxy-server", "LICENSE"})
            self.assertIn(b"Apache License", archive.read("LICENSE"))
            for name in ("muxy", "muxy-server"):
                self.assertEqual(archive.read(name), (output / "Muxy Beta.app/Contents/MacOS" / name).read_bytes())
        self.assertNotEqual(self.build().returncode, 0)

    def test_non_beta_releases_are_rejected_before_packaging(self):
        for version in ("2.0.0", "2.0.0-alpha-1", "2.0.0-beta-0"):
            with self.subTest(version=version):
                result = self.build(version=version)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("release version must be", result.stderr)
                self.assertFalse(self.log.exists())

if __name__ == "__main__":
    unittest.main()
