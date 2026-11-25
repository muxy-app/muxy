#!/usr/bin/env python3
"""Package a Windows release build as muxy-VERSION-windows-ARCH.zip."""

import argparse
from pathlib import Path
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import release  # noqa: E402

BINARIES = ("muxy-app.exe", "muxy.exe", "muxy-server.exe")
ARCHES = ("x86_64", "arm64")


def package(version, arch, binaries, output):
    """Checks each binary reports this release, then zips them into `output`."""
    if arch not in ARCHES:
        raise ValueError(f"unknown Windows architecture {arch}")
    for name in BINARIES:
        release.check_build(version, binaries / name)
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"muxy-{version}-windows-{arch}.zip"
    folder = f"muxy-{version}-windows-{arch}"
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as zipped:
        for name in BINARIES:
            zipped.write(binaries / name, f"{folder}/{name}")
        zipped.write(ROOT / "LICENSE", f"{folder}/LICENSE")
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    parser.add_argument("arch", choices=ARCHES)
    parser.add_argument("binaries", type=Path, help="the folder Cargo built the release into")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    print(package(args.version, args.arch, args.binaries, args.output))


if __name__ == "__main__":
    main()
