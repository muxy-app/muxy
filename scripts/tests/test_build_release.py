import json
import os
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
sys.path.insert(0, str(ROOT / "scripts"))
from beta_release import build_metadata  # noqa: E402

BUILD_INFO = json.dumps({"version": VERSION, **build_metadata(ROOT)})
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
            "scripts/build-release.sh", "scripts/beta_release.py", "scripts/zig/zig",
            "crates/muxy-protocol/src/build.rs", "crates/muxy-protocol/src/version.rs", "LICENSE", "crates/muxy-server/src/detection/THIRD_PARTY.md", "crates/muxy-server/src/detection/LICENSE-herdr",
            "packaging/macos/Muxy.entitlements",
            "packaging/macos/AppIcon.png", "packaging/macos/AppIconBeta.png",
        ):
            destination = self.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, destination)
        for target in TARGETS.values():
            binaries = self.root / "target" / target / "release"
            binaries.mkdir(parents=True)
            for name in ("muxy-app", "muxy", "muxy-server"):
                (binaries / name).write_text(f"#!{sys.executable}\nimport json\nprint(json.dumps({BUILD_INFO}))\n")
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

if __name__ == "__main__":
    unittest.main()
