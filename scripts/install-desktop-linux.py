#!/usr/bin/env python3
"""Install the Linux desktop in the current user's data directory; never run as root."""

import argparse
import contextlib
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request
import urllib.parse

RELEASES = "https://github.com/muxy-app/muxy/releases/download"
MAX_ARCHIVE = 2 * 1024**3
FILES = {"muxy-app", "muxy", "muxy-server", "icon.png", "install.py", "LICENSE", "manifest.json"}
BINARIES = {"muxy-app", "muxy", "muxy-server"}
LOCK = ".muxy-beta-update.lock"


class HTTPSRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, url):
        if urllib.parse.urlsplit(url).scheme != "https":
            raise ValueError("Refusing an insecure download redirect")
        return super().redirect_request(request, response, code, message, headers, url)


def channel(version):
    if re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+-beta\.[1-9][0-9]*", version):
        return "beta"
    if re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        return "stable"
    raise ValueError("Expected X.Y.Z or X.Y.Z-beta.N")


def newer(version, current):
    if channel(version) != channel(current):
        return False
    if channel(version) == "beta":
        return int(version.split("-beta.")[1]) > int(current.split("-beta.")[1])
    return tuple(map(int, version.split("."))) > tuple(map(int, current.split(".")))


def architecture():
    try:
        return {"aarch64": "arm64", "x86_64": "x86_64"}[platform.machine()]
    except KeyError:
        raise ValueError("The Linux desktop requires ARM64 or x86_64") from None


def data_home():
    value = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share"))
    if not value.is_absolute():
        raise ValueError("XDG_DATA_HOME must be absolute")
    return value.resolve()


def default_root(version):
    return data_home() / ("muxy-desktop-beta" if channel(version) == "beta" else "muxy-desktop")


def owned_directory(path):
    metadata = path.lstat()
    if not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != os.getuid() or metadata.st_mode & 0o022:
        raise ValueError(f"Not a private, user-owned installation directory: {path}")


def read_json(path):
    if path.is_symlink() or path.stat().st_size > 65536:
        raise ValueError(f"Invalid metadata file: {path}")
    return json.loads(path.read_text())


def atomic_json(path, value):
    with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as output:
        temporary = Path(output.name)
        try:
            json.dump(value, output)
            output.flush()
            os.fsync(output.fileno())
            os.replace(temporary, path)
        finally:
            temporary.unlink(missing_ok=True)


def sync_directory(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


@contextlib.contextmanager
def locked(root, timeout=0):
    owned_directory(root)
    descriptor = os.open(root / LOCK, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, "r+") as lock:
        deadline = time.monotonic() + timeout
        while True:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise
                time.sleep(0.025)
        yield


def installation(root, version):
    owned_directory(root)
    owned_directory(root / "versions")
    if read_json(root / "installation.json") != {"schema": 1, "channel": channel(version)}:
        raise ValueError("Installation belongs to a different channel or format")


def current_version(root):
    target = os.readlink(root / "current")
    version = target.removeprefix("versions/")
    channel(version)
    if target != f"versions/{version}":
        raise ValueError("The current installation link is invalid")
    return version


def digest(path):
    checksum = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            checksum.update(block)
    return checksum.hexdigest()


def verify(directory, version, arch=None):
    owned_directory(directory)
    manifest = read_json(directory / "manifest.json")
    if (manifest.get("schema") != 1 or manifest.get("version") != version
            or manifest.get("channel") != channel(version)
            or manifest.get("arch") != (arch or architecture())
            or set(manifest.get("files", {})) != FILES - {"manifest.json"}):
        raise ValueError("Desktop package identity does not match this release")
    if {entry.name for entry in directory.iterdir()} - FILES - {".lease"}:
        raise ValueError("Unexpected files in the desktop package")
    for name, expected in manifest["files"].items():
        path = directory / name
        metadata = path.lstat()
        if (not stat.S_ISREG(metadata.st_mode) or metadata.st_size != expected["size"]
                or metadata.st_mode & 0o6022 or digest(path) != expected["sha256"]):
            raise ValueError(f"Desktop package checksum or permissions differ: {name}")
        if name in BINARIES and not metadata.st_mode & stat.S_IXUSR:
            raise ValueError(f"Desktop executable is not executable: {name}")
    build = manifest["build"]
    if build.get("version") != version:
        raise ValueError("Desktop build version mismatch")
    for name in sorted(BINARIES):
        with tempfile.TemporaryFile() as output:
            subprocess.run([str(directory / name), "--build-info"], stdout=output,
                           stderr=subprocess.DEVNULL, stdin=subprocess.DEVNULL, timeout=60, check=True)
            output.seek(0)
            if json.loads(output.read(4097)) != build:
                raise ValueError(f"Desktop app and helpers must be from the same build: {name}")
    return build


def extract(archive, destination, expected):
    if not re.fullmatch(r"[0-9a-f]{64}", expected) or archive.stat().st_size > MAX_ARCHIVE or digest(archive) != expected:
        raise ValueError("Desktop archive checksum or size mismatch")
    with tarfile.open(archive, "r:gz") as source:
        seen = set()
        total = 0
        for member in source:
            name = member.name.removeprefix("muxy-desktop/")
            total += member.size
            if (name not in FILES or member.name != f"muxy-desktop/{name}" or name in seen
                    or not member.isfile() or member.size < 0 or total > MAX_ARCHIVE):
                raise ValueError("Unsafe or unexpected desktop archive entry")
            seen.add(name)
            with source.extractfile(member) as incoming, (destination / name).open("xb") as output:
                shutil.copyfileobj(incoming, output, 1024 * 1024)
                output.flush()
                os.fsync(output.fileno())
            (destination / name).chmod(0o755 if name in BINARIES else 0o644)
        if seen != FILES:
            raise ValueError("Incomplete desktop archive")


def stage(root, archive, version, expected):
    installation(root, version)
    destination = root / "versions" / version
    if destination.exists() or destination.is_symlink():
        verify(destination, version)
        return
    with tempfile.TemporaryDirectory(prefix=".stage-", dir=root) as temporary:
        directory = Path(temporary) / "package"
        directory.mkdir(mode=0o700)
        extract(archive, directory, expected)
        verify(directory, version)
        (directory / ".lease").touch(mode=0o600)
        sync_directory(directory)
        os.rename(directory, destination)
        sync_directory(root / "versions")


def switch(root, version):
    installation(root, version)
    with tempfile.TemporaryDirectory(prefix=".switch-", dir=root) as temporary:
        link = Path(temporary) / "current"
        link.symlink_to(f"versions/{version}")
        os.replace(link, root / "current")
    sync_directory(root)


@contextlib.contextmanager
def lease(directory):
    descriptor = os.open(directory / ".lease", os.O_RDWR | os.O_NOFOLLOW)
    with os.fdopen(descriptor, "r+") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield


def prune(root):
    keep = {current_version(root)}
    previous = root / "previous.json"
    if previous.exists():
        keep.add(read_json(previous)["version"])
    for directory in (root / "versions").iterdir():
        channel(directory.name)
        owned_directory(directory)
        if directory.name in keep or newer(directory.name, current_version(root)):
            continue
        try:
            with lease(directory):
                shutil.rmtree(directory)
        except BlockingIOError:
            pass


def activate(root, version, current, parent, spawn=subprocess.Popen):
    if not newer(version, current) or current_version(root) != current:
        raise ValueError("The installed app changed; reopen it before updating")
    verify(root / "versions" / version, version)
    atomic_json(root / "previous.json", {"version": current})
    try:
        switch(root, version)
        spawn(["/usr/bin/python3", str(Path(__file__).resolve()), "restart", "--root", str(root),
               "--version", version, "--current", current, "--pid", str(parent)],
              stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
              start_new_session=True)
    except BaseException:
        switch(root, current)
        raise


def restart(root, version, previous, parent):
    deadline = time.monotonic() + 120
    while Path(f"/proc/{parent}").exists():
        if time.monotonic() >= deadline:
            with locked(root, timeout=30):
                if current_version(root) == version:
                    switch(root, previous)
            return
        time.sleep(0.1)
    with locked(root, timeout=30):
        if current_version(root) != version:
            return
        try:
            child = subprocess.Popen([str(root / "current/muxy-app")], start_new_session=True)
        except OSError:
            child = None
    if child is not None:
        try:
            if child.wait(timeout=5) == 0:
                return
        except subprocess.TimeoutExpired:
            return
    with locked(root, timeout=30):
        if current_version(root) != version:
            return
        switch(root, previous)
    subprocess.Popen([str(root / "current/muxy-app")], start_new_session=True)


def desktop_entry(root, version, appimage=False):
    beta = channel(version) == "beta"
    identifier = "com.muxy-beta.app" if beta else "com.muxy.app"
    path = data_home() / "applications" / f"{identifier}.desktop"
    executable = str(root / ("usr/bin/muxy-app" if appimage else "current/muxy-app"))
    executable = executable.replace("\\", "\\\\\\\\").replace('"', '\\\\"').replace("`", "\\\\`").replace("$", "\\\\$").replace("%", "%%")
    icon = str(root / (f"{identifier}.png" if appimage else "current/icon.png")).replace("\\", "\\\\")
    contents = (f"[Desktop Entry]\nType=Application\nName={'Muxy Beta' if beta else 'Muxy'}\n"
                f'Exec="{executable}" %F\nIcon={icon}\nTerminal=false\nCategories=Development;TerminalEmulator;\n'
                f"StartupWMClass={identifier}\n")
    if appimage:
        contents += f"X-Muxy-AppImage={root.name}\n"
    return path, contents


def integrate_appimage(root, version):
    if root.parent.resolve() != data_home() / "muxy-appimage" or not re.fullmatch(r"[0-9a-f]{64}", root.name):
        raise ValueError("Invalid unpacked AppImage path")
    owned_directory(root)
    path, contents = desktop_entry(root, version, appimage=True)
    if path.exists() or path.is_symlink():
        previous = path.read_text() if path.is_file() and not path.is_symlink() else ""
        if previous == contents:
            return
        owner = re.search(r"^X-Muxy-AppImage=([0-9a-f]{64})$", previous, re.MULTILINE)
        if not owner or previous != desktop_entry(root.parent / owner[1], version, appimage=True)[1]:
            print(f"Kept existing desktop entry {path}", file=sys.stderr)
            return
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as output:
        temporary = Path(output.name)
        try:
            output.write(contents)
            output.flush()
            os.replace(temporary, path)
        finally:
            temporary.unlink(missing_ok=True)


def integrate(root, version):
    path, contents = desktop_entry(root, version)
    if path.is_symlink() or (path.exists() and path.read_text() != contents):
        raise ValueError(f"Refusing to overwrite an unrelated desktop entry: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(contents)
    commands = Path.home() / ".local/bin"
    commands.mkdir(parents=True, exist_ok=True)
    suffix = "-beta" if channel(version) == "beta" else ""
    for name in ("muxy", "muxy-server"):
        link = commands / (name + suffix)
        target = root / "current" / name
        try:
            link.symlink_to(target)
        except FileExistsError:
            if not link.is_symlink() or os.readlink(link) != str(target):
                print(f"Kept existing command {link}; the bundled command is {target}", file=sys.stderr)


def uninstall(root):
    version = current_version(root)
    installation(root, version)
    with contextlib.ExitStack() as stack:
        for directory in (root / "versions").iterdir():
            channel(directory.name)
            owned_directory(directory)
            if {path.name for path in directory.iterdir()} - FILES - {".lease"}:
                raise ValueError(f"Unrecognized files in {directory}; move them before uninstalling")
            stack.enter_context(lease(directory))
        path, contents = desktop_entry(root, version)
        if path.is_file() and not path.is_symlink() and path.read_text() == contents:
            path.unlink()
        commands = Path.home() / ".local/bin"
        for name in ("muxy", "muxy-server", "muxy-beta", "muxy-server-beta"):
            link = commands / name
            binary = name.removesuffix("-beta")
            if link.is_symlink() and os.readlink(link) == str(root / "current" / binary):
                link.unlink()
        shutil.rmtree(root / "versions")
        for name in ("current", "previous.json", "installation.json", LOCK):
            (root / name).unlink(missing_ok=True)
        if not any(root.iterdir()):
            root.rmdir()


def fetch(url, output, limit):
    request = urllib.request.Request(url, headers={"User-Agent": "Muxy-Desktop-Installer"})
    with urllib.request.build_opener(HTTPSRedirectHandler()).open(request, timeout=60) as response:
        if not response.url.startswith("https://"):
            raise ValueError("Refusing an insecure download redirect")
        total = 0
        while block := response.read(min(1024 * 1024, limit + 1 - total)):
            total += len(block)
            if total > limit:
                raise ValueError("Download exceeds its expected size")
            output.write(block)
    return total


def latest(release_channel):
    import io
    url = (f"{RELEASES}/beta-2.x/update.json" if release_channel == "beta"
           else "https://github.com/muxy-app/muxy/releases/latest/download/update.json")
    metadata = io.BytesIO()
    fetch(url, metadata, 65536)
    feed = json.loads(metadata.getvalue())
    version = feed["version"]
    if feed.get("schema") != 1 or channel(version) != release_channel:
        raise ValueError("Unexpected release channel metadata")
    key = "linux-aarch64" if architecture() == "arm64" else "linux-x86_64"
    if key not in feed.get("platforms", {}):
        raise ValueError(f"The latest {release_channel} release has no Linux desktop for this architecture. "
                         "Use --channel beta to opt into beta releases, or --version to select a desktop release.")
    return version


def download(version, directory):
    import io
    metadata = io.BytesIO()
    fetch(f"{RELEASES}/v{version}/update.json", metadata, 65536)
    feed = json.loads(metadata.getvalue())
    arch = architecture()
    asset = feed["platforms"]["linux-aarch64" if arch == "arm64" else "linux-x86_64"]
    url = f"{RELEASES}/v{version}/muxy-desktop-{version}-linux-{arch}.tar.gz"
    size = asset["size"]
    if feed["schema"] != 1 or feed["version"] != version or asset["url"] != url or not 0 < size <= MAX_ARCHIVE:
        raise ValueError("Unexpected desktop release metadata")
    archive = directory / "desktop.tar.gz"
    with archive.open("xb") as output:
        if fetch(url, output, size) != size:
            raise ValueError("Incomplete desktop download")
    return archive, asset["sha256"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("install", "stage", "verify", "prune", "discard", "activate", "restart", "uninstall", "integrate-appimage"))
    parser.add_argument("--version", help="Exact release; otherwise install the latest release in the selected channel")
    parser.add_argument("--channel", choices=("stable", "beta"), help="Channel for install (default: stable)")
    parser.add_argument("--root", type=Path)
    parser.add_argument("--archive", type=Path)
    parser.add_argument("--sha256")
    parser.add_argument("--current")
    parser.add_argument("--pid", type=int)
    args = parser.parse_args()
    if platform.system() != "Linux" or os.getuid() == 0:
        raise ValueError("Run as your normal user on Linux, without sudo")
    if args.channel and args.command != "install":
        raise ValueError("--channel is only used for install")
    if args.command == "install":
        if args.archive and not args.version:
            raise ValueError("An offline archive requires --version and --sha256")
        if not args.version:
            args.version = latest(args.channel or "stable")
        if args.channel and channel(args.version) != args.channel:
            raise ValueError("The version does not belong to the selected channel")
        print(f"Installing Muxy {args.version} for Linux {architecture()}…", flush=True)
    if args.command in {"install", "stage", "verify", "discard", "activate", "restart", "integrate-appimage"}:
        channel(args.version or "")
    root = args.root or (default_root(args.version) if args.command == "install" else Path(__file__).resolve().parent.parent.parent)
    if not root.is_absolute() or any(character in str(root) for character in "\n\r\0"):
        raise ValueError("Installation path must be absolute and contain no control characters")
    if args.command in {"activate", "restart"}:
        channel(args.current or "")
        if not args.pid or args.pid <= 1:
            raise ValueError("Missing updater process")
    if args.command == "integrate-appimage":
        integrate_appimage(root, args.version)
        return
    if args.command == "restart":
        restart(root, args.version, args.current, args.pid)
        return
    if args.command == "install":
        root.parent.mkdir(parents=True, exist_ok=True)
        if not root.exists():
            root.mkdir(mode=0o700)
            (root / "versions").mkdir(mode=0o700)
            atomic_json(root / "installation.json", {"schema": 1, "channel": channel(args.version)})
    with locked(root):
        if args.command in {"install", "stage"}:
            installation(root, args.version)
            if args.command == "install" and (root / "current").is_symlink():
                raise ValueError("Muxy is already installed. Use Check for Updates in the app; uninstall before reinstalling.")
            with tempfile.TemporaryDirectory() as temporary:
                archive, expected = ((args.archive, args.sha256) if args.archive
                                     else download(args.version, Path(temporary)))
                stage(root, archive, args.version, expected or "")
            if args.command == "install":
                integrate(root, args.version)
                switch(root, args.version)
                print(f"Installed {root / 'current'}. Open Muxy from your desktop's Applications menu.")
        elif args.command == "verify":
            installation(root, args.version)
            print(json.dumps(verify(root / "versions" / args.version, args.version)))
        elif args.command == "prune":
            installation(root, current_version(root))
            prune(root)
        elif args.command == "activate":
            installation(root, args.version)
            activate(root, args.version, args.current, args.pid)
        elif args.command == "discard":
            installation(root, args.version)
            directory = root / "versions" / args.version
            if newer(args.version, current_version(root)) and directory.exists():
                owned_directory(directory)
                with lease(directory):
                    shutil.rmtree(directory)
        else:
            uninstall(root)
            print("Removed the desktop installation. Your Muxy settings, projects and session data were kept.")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError, tarfile.TarError) as error:
        sys.exit(f"Error: {error}")
