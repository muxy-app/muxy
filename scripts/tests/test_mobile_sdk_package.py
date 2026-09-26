import hashlib
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERSION = "2.0.0-beta-1234"
SPEC = importlib.util.spec_from_file_location("beta_release", ROOT / "scripts/beta_release.py")
beta_release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(beta_release)

# What scripts/build-mobile-sdk.sh writes, including intermediate files that are not shipped.
BUILT = [
    "MuxyMobile.xcframework/Info.plist",
    "MuxyMobile.xcframework/ios-arm64/Headers/module.modulemap",
    "MuxyMobile.xcframework/ios-arm64/libmuxy_mobile.a",
    "MuxyMobile.xcframework/ios-arm64_x86_64-simulator/libmuxy_mobile.a",
    "swift/muxy_mobile.swift",
    "swift/muxy_mobileFFI.h",
    "ios/simulator/libmuxy_mobile.a",
    "android/jniLibs/arm64-v8a/libmuxy_mobile.so",
    "android/jniLibs/armeabi-v7a/libmuxy_mobile.so",
    "android/jniLibs/x86_64/libmuxy_mobile.so",
    "android/kotlin/uniffi/muxy_mobile/muxy_mobile.kt",
]


class MobileSdkPackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.sdk = Path(self.temp.name) / "sdk"
        self.output = Path(self.temp.name) / "release"
        for name in BUILT:
            path = self.sdk / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(name)

    def package(self, version=VERSION):
        return subprocess.run(
            [sys.executable, str(ROOT / "scripts/package-mobile-sdk.py"), version, str(self.sdk), str(self.output)],
            capture_output=True, text=True,
        )

    def test_zips_hold_what_each_app_links_and_the_json_pins_them(self):
        result = self.package()
        self.assertEqual(result.returncode, 0, result.stderr)
        ios = self.output / f"muxy-mobile-{VERSION}-ios.zip"
        android = self.output / f"muxy-mobile-{VERSION}-android.zip"
        with zipfile.ZipFile(ios) as archive:
            self.assertEqual(sorted(archive.namelist()), [
                "MuxyMobile.xcframework/Info.plist",
                "MuxyMobile.xcframework/ios-arm64/Headers/module.modulemap",
                "MuxyMobile.xcframework/ios-arm64/libmuxy_mobile.a",
                "MuxyMobile.xcframework/ios-arm64_x86_64-simulator/libmuxy_mobile.a",
                "muxy_mobile.swift",
            ])
            self.assertEqual(archive.read("muxy_mobile.swift"), b"swift/muxy_mobile.swift")
        with zipfile.ZipFile(android) as archive:
            self.assertEqual(sorted(archive.namelist()), [
                "jniLibs/arm64-v8a/libmuxy_mobile.so",
                "jniLibs/armeabi-v7a/libmuxy_mobile.so",
                "jniLibs/x86_64/libmuxy_mobile.so",
                "muxy_mobile.kt",
            ])
        metadata = json.loads((self.output / f"muxy-mobile-{VERSION}.json").read_text())
        self.assertEqual(metadata, {
            "version": VERSION,
            **beta_release.build_metadata(),
            "sha256": {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in (ios, android)},
        })
        self.assertEqual(len(list(self.output.iterdir())), 3)

    def test_incomplete_build_is_rejected_before_anything_is_written(self):
        (self.sdk / "android/jniLibs/x86_64/libmuxy_mobile.so").unlink()
        result = self.package()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("android/jniLibs/x86_64/libmuxy_mobile.so", result.stderr)
        self.assertFalse(self.output.exists())

    def test_only_beta_versions_are_packaged(self):
        result = self.package("2.0.0")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("2.0.0-beta-", result.stderr)
        self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()
