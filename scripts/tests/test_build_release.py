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
VERSION = "2.0.0-beta.1234"
STABLE = "2.0.0"
sys.path.insert(0, str(ROOT / "scripts"))
from release import build_metadata  # noqa: E402

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
            "scripts/build-release.sh", "scripts/release.py", "scripts/zig/zig",
            "crates/muxy-protocol/src/build.rs", "crates/muxy-protocol/src/version.rs", "LICENSE", "crates/muxy-server/src/detection/THIRD_PARTY.md", "crates/muxy-server/src/detection/LICENSE-herdr",
            "packaging/macos/Muxy.entitlements",
            "packaging/macos/AppIcon.png", "packaging/macos/AppIconBeta.png",
        ):
            destination = self.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, destination)
        self.binaries(VERSION)
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

    def binaries(self, version):
        build_info = json.dumps({"version": version, **build_metadata(ROOT)})
        for target in TARGETS.values():
            binaries = self.root / "target" / target / "release"
            for name in ("muxy-app", "muxy", "muxy-server"):
                (binaries / f"{name}.dSYM").mkdir(parents=True, exist_ok=True)
                (binaries / name).write_text(f"#!{sys.executable}\nimport json\nprint(json.dumps({build_info}))\n")

    def build(self, arch="arm64", version=VERSION, *extra):
        return subprocess.run(
            ["bash", str(self.root / "scripts/build-release.sh"),
             "--arch", arch, "--version", version, *extra],
            env={**self.env, "TEST_ARCH": arch}, capture_output=True, text=True,
        )

    def calls(self, tool):
        return [entry for line in self.log.read_text().splitlines()
                if (entry := json.loads(line))[0] == tool]

    def test_standalone_zip_contains_exact_bundle_executable_bytes(self):
        result = self.build()
        self.assertEqual(result.returncode, 0, result.stderr)
        output = self.root / "target/packages" / VERSION / "arm64"
        with zipfile.ZipFile(output / f"muxy-{VERSION}-macos-arm64.zip") as archive:
            self.assertEqual(set(archive.namelist()), {"muxy", "muxy-server", "LICENSE"})
            self.assertIn(b"Apache License", archive.read("LICENSE"))
            for name in ("muxy", "muxy-server"):
                self.assertEqual(archive.read(name), (output / "Muxy Beta.app/Contents/MacOS" / name).read_bytes())
        self.assertNotEqual(self.build().returncode, 0)

    def test_stable_builds_the_app_that_replaces_muxy_1x(self):
        self.binaries(STABLE)
        self.assertNotEqual(self.build("arm64", STABLE).returncode, 0)
        result = self.build("arm64", STABLE, "--build-number", "1234")
        self.assertEqual(result.returncode, 0, result.stderr)
        app = self.root / "target/packages" / STABLE / "arm64/Muxy.app"
        info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
        self.assertEqual(
            [info[key] for key in ("CFBundleName", "CFBundleIdentifier", "CFBundleVersion", "MuxyVersion")],
            ["Muxy", "com.muxy.app", "1234", STABLE],
        )
        self.assertTrue(any(call[-1].endswith("Muxy-2.0.0-arm64.dmg") and "Muxy" in call
                            for call in self.calls("hdiutil")))
        self.assertTrue(all(call[4].endswith("/AppIcon.png") for call in self.calls("sips")))


if __name__ == "__main__":
    unittest.main()
