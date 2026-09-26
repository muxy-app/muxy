#!/usr/bin/env python3
"""Package a scripts/build-mobile-sdk.sh build as the mobile SDK files of a beta release."""

import argparse
import hashlib
import json
from pathlib import Path
import zipfile

from beta_release import build_metadata, build_number

ANDROID_ABIS = ("arm64-v8a", "armeabi-v7a", "x86_64")


def contents(sdk):
    """Each zip's entries as (name in the zip, built file); fails if the build is incomplete."""
    framework = sdk / "MuxyMobile.xcframework"
    ios = [(path.relative_to(sdk).as_posix(), path)
           for path in sorted(framework.rglob("*")) if path.is_file()]
    ios.append(("muxy_mobile.swift", sdk / "swift/muxy_mobile.swift"))
    android = [(f"jniLibs/{abi}/libmuxy_mobile.so", sdk / "android/jniLibs" / abi / "libmuxy_mobile.so")
               for abi in ANDROID_ABIS]
    android.append(("muxy_mobile.kt", sdk / "android/kotlin/uniffi/muxy_mobile/muxy_mobile.kt"))
    required = [framework / "Info.plist", *(source for _, source in ios + android)]
    missing = [str(path) for path in required if not path.is_file()]
    if missing:
        raise ValueError("incomplete SDK build; missing " + ", ".join(missing))
    return {"ios": ios, "android": android}


def package(version, sdk, output):
    build_number(version)
    build = build_metadata()
    zips = contents(sdk)
    output.mkdir(parents=True, exist_ok=True)
    checksums = {}
    for platform, entries in zips.items():
        name = f"muxy-mobile-{version}-{platform}.zip"
        with zipfile.ZipFile(output / name, "w", zipfile.ZIP_DEFLATED) as archive:
            for entry, source in entries:
                archive.write(source, entry)
        checksums[name] = hashlib.sha256((output / name).read_bytes()).hexdigest()
    metadata = {"version": version, **build, "sha256": checksums}
    (output / f"muxy-mobile-{version}.json").write_text(json.dumps(metadata, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    parser.add_argument("sdk", type=Path, help="the folder scripts/build-mobile-sdk.sh wrote")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    try:
        package(args.version, args.sdk, args.output)
    except (ValueError, OSError) as error:
        parser.exit(1, f"error: {error}\n")
    print(f"Mobile SDK {args.version} packaged in {args.output}")


if __name__ == "__main__":
    main()
