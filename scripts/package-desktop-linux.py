#!/usr/bin/env python3
"""Package one native Linux desktop build, including matching CLI and server helpers."""

import argparse
import hashlib
import json
import shutil
import tarfile
import tempfile
from pathlib import Path

from release import ROOT, build_metadata, channel, check_build


def package(version, arch, binaries, output, native_formats=False, tools=None):
    release_channel = channel(version)
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"muxy-desktop-{version}-linux-{arch}.tar.gz"
    if archive.exists():
        raise ValueError(f"Output already exists: {archive}")
    with tempfile.TemporaryDirectory() as temporary:
        directory = Path(temporary)
        for name in ("muxy-app", "muxy", "muxy-server"):
            check_build(version, binaries / name)
            shutil.copyfile(binaries / name, directory / name)
            (directory / name).chmod(0o755)
        icon = "AppIconBeta.png" if release_channel == "beta" else "AppIcon.png"
        shutil.copyfile(ROOT / "packaging/macos" / icon, directory / "icon.png")
        shutil.copyfile(ROOT / "scripts/install-desktop-linux.py", directory / "install.py")
        (directory / "LICENSE").write_text("\n".join(
            (ROOT / path).read_text() for path in (
                "LICENSE", "crates/muxy-server/src/detection/THIRD_PARTY.md",
                "crates/muxy-server/src/detection/LICENSE-herdr")))
        files = {}
        for path in sorted(directory.iterdir()):
            checksum = hashlib.sha256()
            with path.open("rb") as source:
                for block in iter(lambda: source.read(1024 * 1024), b""):
                    checksum.update(block)
            files[path.name] = {"size": path.stat().st_size, "sha256": checksum.hexdigest()}
        (directory / "manifest.json").write_text(json.dumps({
            "schema": 1, "version": version, "channel": release_channel, "arch": arch,
            "build": {"version": version, **build_metadata()}, "files": files,
            "requirements": {"glibc": "2.39", "webkitgtk": "4.1 >= 2.40", "python": "3.9",
                             "ubuntu_packages": ["libfreetype6", "libfontconfig1", "libxcb1", "libxkbcommon0",
                                                 "libxkbcommon-x11-0", "libwayland-client0", "libvulkan1",
                                                 "libwebkit2gtk-4.1-0", "xdg-utils", "xdg-desktop-portal",
                                                 "python3", "ca-certificates", "bubblewrap", "xdg-dbus-proxy"]},
        }, indent=2) + "\n")
        with tarfile.open(archive, "w:gz", format=tarfile.USTAR_FORMAT) as target:
            for path in sorted(directory.iterdir()):
                info = target.gettarinfo(path, arcname=f"muxy-desktop/{path.name}")
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                info.mtime = 0
                info.mode = 0o755 if path.name in {"muxy-app", "muxy", "muxy-server"} else 0o644
                with path.open("rb") as source:
                    target.addfile(info, source)
        if native_formats:
            from linux_packages import package_appimage, package_deb
            package_deb(version, arch, directory, output)
            package_appimage(version, arch, directory, output, tools or output / "appimage-tools")
    print(archive)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    parser.add_argument("arch", choices=("arm64", "x86_64"))
    parser.add_argument("binaries", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--native-formats", action="store_true", help="Also build Debian and AppImage packages on Ubuntu 24.04")
    parser.add_argument("--tools", type=Path, help="Cache of checksummed AppImage build tools")
    args = parser.parse_args()
    package(args.version, args.arch, args.binaries, args.output, args.native_formats, args.tools)
