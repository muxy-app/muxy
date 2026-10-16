"""Native Linux desktop formats, built from the same payload as the user-local archive."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

from release import APPS, ROOT, channel, check_build


ARCHITECTURES = {"arm64": ("arm64", "aarch64"), "x86_64": ("amd64", "x86_64")}


def identity(version):
    release_channel = channel(version)
    name, identifier = APPS[release_channel]
    suffix = "-beta" if release_channel == "beta" else ""
    return name, identifier, suffix


def desktop_entry(version, executable):
    name, identifier, _ = identity(version)
    return (f"[Desktop Entry]\nType=Application\nName={name}\n"
            f"Exec={executable} %F\nIcon={identifier}\nTerminal=false\n"
            f"Categories=Development;TerminalEmulator;\nStartupWMClass={identifier}\n")


def copy(source, destination, mode=0o644):
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.is_symlink():
        destination.unlink()
    shutil.copyfile(source, destination)
    destination.chmod(mode)


def write(destination, contents, mode=0o644):
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(contents)
    destination.chmod(mode)


def debian_version(version):
    channel(version)
    return version.replace("-beta.", "~beta.")


def package_deb(version, arch, payload, output):
    name, identifier, suffix = identity(version)
    package = f"muxy-desktop{suffix}"
    deb_arch, _ = ARCHITECTURES[arch]
    archive = output / f"{package}_{debian_version(version)}_{deb_arch}.deb"
    if archive.exists():
        raise ValueError(f"Output already exists: {archive}")
    requirements = json.loads((payload / "manifest.json").read_text())["requirements"]
    dependencies = ["libc6 (>= 2.39)", "libgcc-s1", "libwebkit2gtk-4.1-0 (>= 2.42)"]
    dependencies.extend(item for item in requirements["ubuntu_packages"]
                        if item not in {"python3", "libwebkit2gtk-4.1-0"})
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        directory = root / "usr/lib" / package
        for binary in ("muxy-app", "muxy", "muxy-server"):
            copy(payload / binary, directory / binary, 0o755)
        commands = root / "usr/bin"
        commands.mkdir(parents=True)
        for command, binary in ((package, "muxy-app"), (f"muxy{suffix}", "muxy"),
                                (f"muxy-server{suffix}", "muxy-server")):
            (commands / command).symlink_to(f"../lib/{package}/{binary}")
        write(root / f"usr/share/applications/{identifier}.desktop", desktop_entry(version, package))
        copy(payload / "icon.png", root / f"usr/share/pixmaps/{identifier}.png")
        copy(payload / "LICENSE", root / f"usr/share/doc/{package}/copyright")
        size = sum(path.stat().st_size for path in root.rglob("*") if path.is_file() and not path.is_symlink())
        write(root / "DEBIAN/control", (
            f"Package: {package}\nVersion: {debian_version(version)}\nArchitecture: {deb_arch}\n"
            "Section: utils\nPriority: optional\nMaintainer: Saeed Vaziry <me@saeedvaziry.com>\n"
            f"Installed-Size: {(size + 1023) // 1024}\nDepends: {', '.join(dependencies)}\n"
            "Recommends: mesa-vulkan-drivers, xdg-desktop-portal-gnome | xdg-desktop-portal-kde | xdg-desktop-portal-gtk\n"
            "Homepage: https://muxy.app\n"
            f"Description: {name} terminal multiplexer\n"
            " Persistent terminals, projects, split panes and remote sessions.\n"
            " Built for Ubuntu 24.04. Updates are managed by the package manager.\n"))
        guard = (ROOT / "packaging/linux/deb-guard.sh").read_text().replace("@PACKAGE@", package)
        for action in ("preinst", "prerm"):
            write(root / "DEBIAN" / action, guard, 0o755)
        subprocess.run(["dpkg-deb", "--root-owner-group", "--build", str(root), str(archive)], check=True)
    print(archive)


def appimage_tools(arch, directory):
    _, machine = ARCHITECTURES[arch]
    pins = json.loads((ROOT / "packaging/linux/appimage-tools.json").read_text())
    directory.mkdir(parents=True, exist_ok=True)
    tools = {}
    for name, pin in pins.items():
        target = directory / f"{name}-{machine}"
        expected = pin["sha256"][machine]
        if not target.exists() or digest(target) != expected:
            with tempfile.TemporaryDirectory(dir=directory) as temporary:
                download = Path(temporary) / "download"
                subprocess.run(["curl", "--fail", "--location", "--proto", "=https", "--proto-redir", "=https",
                                "--max-time", "300", pin["url"].replace("{arch}", machine), "-o", str(download)], check=True)
                if digest(download) != expected:
                    raise ValueError(f"Checksum mismatch for {name}; refusing to execute downloaded tooling")
                download.chmod(0o755)
                os.replace(download, target)
        tools[name] = target.resolve()
    return tools


def extract_tool(tool, directory):
    directory.mkdir()
    subprocess.run([str(tool), "--appimage-extract"], cwd=directory, check=True, stdout=subprocess.DEVNULL)
    return directory / "squashfs-root"


def digest(path):
    checksum = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            checksum.update(block)
    return checksum.hexdigest()


def bundle_identity(directory):
    checksum = hashlib.sha256()
    for path in sorted(directory.rglob("*")):
        if path.is_symlink():
            contents = os.readlink(path).encode()
        elif path.is_file():
            contents = digest(path).encode()
        else:
            continue
        checksum.update(str(path.relative_to(directory)).encode() + b"\0")
        checksum.update(str(path.lstat().st_mode).encode() + b"\0" + contents + b"\0")
    return checksum.hexdigest()


def package_appimage(version, arch, payload, output, tools_directory):
    _, identifier, _ = identity(version)
    _, machine = ARCHITECTURES[arch]
    archive = output / f"Muxy-{version}-linux-{arch}.AppImage"
    if archive.exists():
        raise ValueError(f"Output already exists: {archive}")
    tools = appimage_tools(arch, tools_directory)
    with tempfile.TemporaryDirectory() as temporary:
        work = Path(temporary)
        root = work / "Muxy.AppDir"
        binaries = root / "usr/bin"
        for binary in ("muxy-app", "muxy", "muxy-server"):
            copy(payload / binary, binaries / binary, 0o755)
        copy(payload / "LICENSE", root / "usr/share/doc/muxy/LICENSE")
        copy(ROOT / "packaging/linux/AppImage-LICENSE", root / "usr/share/doc/appimage-runtime/LICENSE")
        copy(payload / "icon.png", root / f"{identifier}.png")
        (root / ".DirIcon").symlink_to(f"{identifier}.png")
        write(root / f"{identifier}.desktop", desktop_entry(version, "muxy-app"))
        deploy = extract_tool(tools["linuxdeploy"], work / "linuxdeploy")
        appimagetool = extract_tool(tools["appimagetool"], work / "appimagetool")
        command = [str(deploy / "AppRun"), "--appdir", str(root)]
        for binary in binaries.iterdir():
            command.extend(["--executable", str(binary)])
        env = {**os.environ, "NO_STRIP": "1", "ARCH": machine}
        subprocess.run(command, env=env, check=True)
        patchelf = deploy / "usr/bin/patchelf"
        for path in root.rglob("*"):
            if not path.is_file() or path.is_symlink():
                continue
            with path.open("rb") as source:
                elf = source.read(4) == b"\x7fELF"
            if elf:
                relative = os.path.relpath(root / "usr/lib", path.parent)
                subprocess.run([str(patchelf), "--set-rpath", f"$ORIGIN/{relative}", str(path)], check=True)
        for binary in ("muxy-app", "muxy", "muxy-server"):
            check_build(version, binaries / binary)
        owners = set()
        for path in (root / "usr/lib").glob("*.so*"):
            found = subprocess.run(["dpkg-query", "-S", f"*/{path.name}"], capture_output=True, text=True, check=False)
            if found.returncode:
                raise ValueError(f"Cannot determine the license of bundled library {path.name}")
            owners.update(line.split(": ", 1)[0] for line in found.stdout.splitlines())
        sources = []
        for owner in sorted(owners):
            package = owner.split(":", 1)[0]
            copy(Path("/usr/share/doc") / package / "copyright", root / "usr/share/doc" / package / "copyright")
            source, source_version = subprocess.check_output(
                ["dpkg-query", "-W", "-f=${source:Package}\t${source:Version}", owner], text=True).split("\t")
            sources.append({"package": owner, "source": source, "version": source_version,
                            "url": f"https://launchpad.net/ubuntu/+source/{source}/{source_version}"})
        shutil.copytree("/usr/share/common-licenses", root / "usr/share/common-licenses")
        write(root / "usr/share/doc/muxy/bundled-sources.json", json.dumps(sources, indent=2) + "\n")
        copy(ROOT / "packaging/linux/AppRun", root / "AppRun", 0o755)
        copy(payload / "install.py", root / "install.py")
        write(root / "version", version + "\n")
        write(root / "bundle-id", bundle_identity(root) + "\n")
        subprocess.run([str(appimagetool / "AppRun"), "--no-appstream", "--runtime-file", str(tools["runtime"]),
                        str(root), str(archive)], env=env, check=True)
    print(archive)
