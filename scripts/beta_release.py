#!/usr/bin/env python3
import argparse
import json
import plistlib
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSION_PATTERN = re.compile(r"2\.0\.0-beta-([1-9][0-9]*)")


def build_metadata(root=ROOT):
    """The --build-info every executable of this checkout reports."""
    build = (root / "crates/muxy-protocol/src/build.rs").read_text()
    version = (root / "crates/muxy-protocol/src/version.rs").read_text()
    current = re.search(r"pub const CURRENT: Version = (V\d+);", version)[1]
    number = int(re.search(rf"pub const {current}: Version = Version\((\d+)\);", version)[1])
    return {
        "compatibility": int(re.search(r"pub const COMPATIBILITY: u64 = (\d+);", build)[1]),
        "protocol": [number],
    }


def build_number(version):
    match = VERSION_PATTERN.fullmatch(version)
    if not match:
        raise ValueError("release version must be 2.0.0-beta-<positive commit count>")
    return match[1]


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], text=True).strip()


def checkout_version(root):
    if git(root, "rev-parse", "--is-shallow-repository") != "false":
        raise ValueError("beta numbering requires a full checkout (fetch-depth: 0)")
    version = "2.0.0-beta-" + git(root, "rev-list", "--count", "HEAD")
    build_number(version)
    return version


def replace_once(pattern, replacement, text, label):
    updated, count = re.subn(pattern, replacement, text, flags=re.MULTILINE)
    if count != 1:
        raise ValueError(f"expected exactly one {label}, found {count}")
    return updated


def stamp_version(root, version):
    build_number(version)
    manifest_path = root / "Cargo.toml"
    lock_path = root / "Cargo.lock"
    manifest = manifest_path.read_text()
    pattern = r'(\[workspace\.package\]\nversion = ")([^"\n]+)(")'
    current = re.search(pattern, manifest)
    if not current:
        raise ValueError("workspace package version not found")
    manifest = replace_once(
        pattern, lambda match: match[1] + version + match[3], manifest, "workspace version"
    )
    lock = lock_path.read_text()
    manifests = sorted(root.glob("crates/*/Cargo.toml"))
    if not manifests:
        raise ValueError("workspace crates not found")
    for crate in manifests:
        name = re.search(r'^name = "([^"]+)"$', crate.read_text(), re.MULTILINE)
        if not name:
            raise ValueError(f"package name not found in {crate}")
        pattern = (
            r'(\[\[package\]\]\nname = "' + re.escape(name[1])
            + r'"\nversion = ")' + re.escape(current[2]) + r'(")'
        )
        lock = replace_once(
            pattern, lambda match: match[1] + version + match[2], lock, name[1]
        )
    # Only local package versions change. All third-party resolutions stay locked.
    manifest_path.write_text(manifest)
    lock_path.write_text(lock)


def bundle_info(version):
    return {
        "CFBundleDevelopmentRegion": "en",
        "CFBundleDisplayName": "Muxy Beta",
        "CFBundleName": "Muxy Beta",
        "CFBundleExecutable": "muxy-app",
        "CFBundleIdentifier": "com.muxy-beta.app",
        "CFBundleIconFile": "AppIcon",
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": "2.0.0",
        "CFBundleVersion": build_number(version),
        "CFBundleGetInfoString": f"Muxy Beta {version}",
        "MuxyVersion": version,
        "LSMinimumSystemVersion": "14.0",
        "LSApplicationCategoryType": "public.app-category.developer-tools",
        "NSHighResolutionCapable": True,
        "NSSupportsAutomaticGraphicsSwitching": True,
        "NSPrincipalClass": "NSApplication",
        "NSMicrophoneUsageDescription": "Muxy uses your microphone to dictate text into Composer.",
        "NSSpeechRecognitionUsageDescription": "Muxy transcribes your dictation on this device and inserts it into Composer.",
    }


def update_metadata(version, repository, directory):
    build_number(version)
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("invalid GitHub repository")
    platforms = {}
    for platform, arch in (("macos-aarch64", "arm64"), ("macos-x86_64", "x86_64")):
        filename = f"Muxy-{version}-{arch}.dmg"
        if arch == "x86_64" and not (directory / filename).exists():
            continue
        size = (directory / filename).stat().st_size
        if not 0 < size <= 2 * 1024**3:
            raise ValueError(f"invalid update size for {arch}")
        platforms[platform] = {
            "url": f"https://github.com/{repository}/releases/download/v{version}/{filename}",
            "size": size,
        }
    return {"schema": 1, "version": version, "platforms": platforms}


def check_build(version, executable):
    build_number(version)
    metadata = json.loads(subprocess.check_output([str(executable), "--build-info"], timeout=5))
    if metadata != {"version": version, **build_metadata()}:
        raise ValueError("Packaged executable build metadata does not match this release")


def main():
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("version")
    check = commands.add_parser("check-build")
    check.add_argument("version")
    check.add_argument("executable", type=Path)
    metadata = commands.add_parser("update")
    metadata.add_argument("version")
    metadata.add_argument("repository")
    metadata.add_argument("directory", type=Path)
    for command in ("stamp", "check-version", "plist"):
        subparser = commands.add_parser(command)
        subparser.add_argument("version")
        if command == "plist":
            subparser.add_argument("output", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "version":
            version = checkout_version(ROOT)
            print(f"version={version}")
            print(f"tag=v{version}")
        elif args.command == "check-build":
            check_build(args.version, args.executable)
        elif args.command == "stamp":
            stamp_version(ROOT, args.version)
        elif args.command == "check-version":
            build_number(args.version)
        elif args.command == "update":
            metadata = update_metadata(args.version, args.repository, args.directory)
            (args.directory / "update.json").write_text(json.dumps(metadata, indent=2) + "\n")
        else:
            args.output.write_bytes(plistlib.dumps(bundle_info(args.version)))
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
