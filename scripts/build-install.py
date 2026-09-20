#!/usr/bin/env python3
"""Build this checkout for the current machine, notarize on macOS, and install."""

import argparse
import contextlib
import fcntl
import json
import os
import platform
import plistlib
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BINARIES = ("muxy", "muxy-server")


def load_release_env(path):
    """Load literal dotenv assignments without replacing exported shell variables."""
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError:
        return
    values = {}
    for number, line in enumerate(lines, 1):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        match = re.fullmatch(r"(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)", line)
        if not match:
            raise ValueError(f"Invalid environment assignment in {path.name} at line {number}.")
        key, value = match.groups()
        try:
            if value.startswith(("'", '"')):
                tokens = shlex.split(value, comments=True, posix=True)
                if len(tokens) != 1:
                    raise ValueError
                value = tokens[0]
            else:
                value = "" if value.startswith("#") else re.split(r"\s+#", value, maxsplit=1)[0].rstrip()
            if "\0" in value:
                raise ValueError
        except ValueError:
            raise ValueError(f"Invalid environment value in {path.name} at line {number}.") from None
        values[key] = value
    for key, value in values.items():
        os.environ.setdefault(key, value)


def run(*args, **kwargs):
    return subprocess.run([str(arg) for arg in args], check=True, **kwargs)


def output(*args):
    return run(*args, capture_output=True, text=True).stdout.strip()


def host_target():
    system = platform.system()
    arch = {"arm64": "aarch64", "aarch64": "aarch64", "x86_64": "x86_64"}.get(platform.machine())
    if not arch or system not in ("Darwin", "Linux"):
        raise ValueError("Supported hosts: macOS and glibc Linux on ARM64 or x86_64.")
    if system == "Darwin":
        if int(platform.mac_ver()[0].split(".")[0]) < 14:
            raise ValueError("macOS 14 or newer is required.")
        return system, arch + "-apple-darwin"
    libc = output("getconf", "GNU_LIBC_VERSION")
    match = re.fullmatch(r"glibc (\d+)\.(\d+)", libc)
    if not match or tuple(map(int, match.groups())) < (2, 35):
        raise ValueError("Linux requires glibc 2.35 or newer; musl is unsupported.")
    return system, arch + "-unknown-linux-gnu"


def signing_identity(requested):
    identities = re.findall(
        r'([A-Fa-f0-9]{40}) "(Developer ID Application: [^"]+)"',
        output("security", "find-identity", "-v", "-p", "codesigning"),
    )
    matches = [digest for digest, name in identities if not requested or requested in (digest, name)]
    if len(matches) != 1:
        raise ValueError("Select one installed Developer ID Application certificate with --sign-identity.")
    return matches[0]


def package_app(directory, binaries, version, identity):
    app = directory / "Muxy Beta.app"
    contents = app / "Contents"
    executables = contents / "MacOS"
    resources = contents / "Resources"
    executables.mkdir(parents=True)
    resources.mkdir()
    for name in (*BINARIES, "muxy-app"):
        shutil.copy2(binaries / name, executables / name)
    beta = re.fullmatch(r"2\.0\.0-beta-(\d+)", version)
    if not beta:
        raise ValueError("Expected the checkout's version to be 2.0.0-beta-N.")
    # Keep local beta-0 builds identifiable without stamping the source checkout.
    from beta_release import bundle_info
    info = bundle_info("2.0.0-beta-" + str(max(1, int(beta[1]))))
    info.update(CFBundleVersion=beta[1], MuxyVersion=version,
                CFBundleGetInfoString="Muxy Beta " + version)
    (contents / "Info.plist").write_bytes(plistlib.dumps(info))
    (contents / "PkgInfo").write_bytes(b"APPL????")
    (resources / "LICENSE").write_bytes(b"\n".join((ROOT / path).read_bytes() for path in (
        "LICENSE", "crates/muxy-server/src/detection/THIRD_PARTY.md",
        "crates/muxy-server/src/detection/LICENSE-herdr",
    )))
    iconset = directory / "AppIcon.iconset"
    iconset.mkdir()
    for size in (16, 32, 128, 256, 512):
        for scale, suffix in ((1, ""), (2, "@2x")):
            run("sips", "-z", size * scale, size * scale,
                ROOT / "packaging/macos/AppIconBeta.png", "--out",
                iconset / f"icon_{size}x{size}{suffix}.png", stdout=subprocess.DEVNULL)
    run("iconutil", "--convert", "icns", "--output", resources / "AppIcon.icns", iconset)
    sign = ("codesign", "--force", "--sign", identity, "--options", "runtime", "--timestamp")
    for name in BINARIES:
        run(*sign, executables / name)
    entitlements = ("--entitlements", ROOT / "packaging/macos/Muxy.entitlements")
    run(*sign, *entitlements, executables / "muxy-app")
    run(*sign, *entitlements, app)
    run("codesign", "--verify", "--deep", "--strict", "--verbose=2", app)
    return app


def remove(path):
    if path.is_symlink() or path.is_file():
        path.unlink()
    elif path.exists():
        shutil.rmtree(path)


def install(artifact, bin_dir, app_dir=None):
    """Stage replacements on their destination filesystem and roll back failures."""
    bin_dir.mkdir(parents=True, exist_ok=True)
    with (bin_dir / ".muxy-build-install.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        destination = app_dir / "Muxy Beta.app" if app_dir else bin_dir / ".muxy-local"
        destination.parent.mkdir(parents=True, exist_ok=True)
        if destination.is_symlink() or (destination.exists() and not destination.is_dir()):
            raise ValueError(f"Expected a regular installation directory: {destination}")
        for name in BINARIES:
            path = bin_dir / name
            if path.exists() and not path.is_file() and not path.is_symlink():
                raise ValueError(f"Cannot replace a directory or special file: {path}")
        with contextlib.ExitStack() as stack:
            stage = Path(stack.enter_context(tempfile.TemporaryDirectory(
                prefix=".muxy-install-", dir=destination.parent)))
            links = Path(stack.enter_context(tempfile.TemporaryDirectory(
                prefix=".muxy-links-", dir=bin_dir)))
            staged = stage / destination.name
            if app_dir:
                # Preserve the signature, resources and stapled notarization ticket.
                run("ditto", artifact, staged)
                run("xcrun", "stapler", "validate", staged)
                run("codesign", "--verify", "--deep", "--strict", staged)
                executable_dir = destination / "Contents/MacOS"
            else:
                shutil.copytree(artifact, staged)
                executable_dir = destination
            for name in BINARIES:
                (links / name).symlink_to(executable_dir / name)
                existing = bin_dir / name
                if existing.exists() or existing.is_symlink():
                    shutil.copy2(existing, links / ("old-" + name), follow_symlinks=False)
            previous = stage / "previous"
            installed = False
            changed = []
            try:
                if destination.exists():
                    destination.rename(previous)
                staged.rename(destination)
                installed = True
                for name in BINARIES:
                    os.replace(links / name, bin_dir / name)
                    changed.append(name)
            except BaseException:
                for name in reversed(changed):
                    saved = links / ("old-" + name)
                    if saved.exists() or saved.is_symlink():
                        os.replace(saved, bin_dir / name)
                    else:
                        (bin_dir / name).unlink()
                if installed:
                    remove(destination)
                if previous.exists():
                    previous.rename(destination)
                raise
    print(f"Installed {destination}\nCLI and server: {bin_dir}")


def main():
    load_release_env(ROOT / ".env.release")
    parser = argparse.ArgumentParser(description=__doc__, epilog=(
        "Loads literal KEY=VALUE assignments from the optional project-root .env.release; "
        "existing shell variables and command-line options take precedence. "
        "macOS credentials: set NOTARY_KEYCHAIN_PROFILE, or APPLE_ID, "
        "APPLE_APP_SPECIFIC_PASSWORD and APPLE_TEAM_ID. Replaces the installed "
        "Muxy bundle and commands. Does not launch the app or restart servers."
    ))
    parser.add_argument("--install-dir", type=Path, default=Path.home() / ".local/bin")
    parser.add_argument("--app-dir", type=Path, default=Path("/Applications"))
    parser.add_argument("--sign-identity", default=os.environ.get("SIGN_IDENTITY"))
    parser.add_argument("--keychain-profile", default=os.environ.get("NOTARY_KEYCHAIN_PROFILE"))
    parser.add_argument("--build-only", action="store_true", help="Build and sign, without notarizing or installing.")
    args = parser.parse_args()
    system, target = host_target()
    for tool in ("cargo", "zig", "python3") + (("xcrun", "codesign", "security", "ditto", "sips", "iconutil") if system == "Darwin" else ()):
        if not shutil.which(tool):
            raise ValueError(f"Required build tool is missing: {tool}")
    identity = signing_identity(args.sign_identity) if system == "Darwin" else None
    if system == "Darwin" and not args.build_only and not args.keychain_profile:
        for key in ("APPLE_ID", "APPLE_APP_SPECIFIC_PASSWORD", "APPLE_TEAM_ID"):
            if not os.environ.get(key):
                raise ValueError(f"Set --keychain-profile or {key} before building.")
    env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "target"),
               LIBGHOSTTY_VT_SYS_OPTIMIZE="ReleaseFast")
    if system == "Darwin":
        env["MACOSX_DEPLOYMENT_TARGET"] = "14.0"
    packages = ["muxy-cli", "muxy-server"] + (["muxy-app"] if system == "Darwin" else [])
    print(f"==> Building for {target}", flush=True)
    run("cargo", "build", "--locked", "--release", "--target", target,
        *(arg for package in packages for arg in ("-p", package)), cwd=ROOT, env=env)
    binaries = ROOT / "target" / target / "release"
    metadata = [json.loads(output(binaries / name, "--build-info")) for name in BINARIES]
    if metadata[0] != metadata[1]:
        raise ValueError("Built CLI and server versions do not match.")
    builds = ROOT / "target/local-install"
    builds.mkdir(parents=True, exist_ok=True)
    directory = Path(tempfile.mkdtemp(prefix=target + "-", dir=builds))
    if system == "Darwin":
        artifact = package_app(directory, binaries, metadata[0]["version"], identity)
        if not args.build_only:
            if args.keychain_profile:
                env["NOTARY_KEYCHAIN_PROFILE"] = args.keychain_profile
            run("bash", ROOT / "scripts/notarize-release.sh", artifact, env=env)
    else:
        artifact = directory / "bin"
        artifact.mkdir()
        for name in BINARIES:
            shutil.copy2(binaries / name, artifact / name)
    print(f"Built {artifact}", flush=True)
    if not args.build_only:
        install(artifact, args.install_dir.expanduser().resolve(),
                args.app_dir.expanduser().resolve() if system == "Darwin" else None)
        if str(args.install_dir.expanduser().resolve()) not in os.environ.get("PATH", "").split(os.pathsep):
            print(f"Add {args.install_dir.expanduser().resolve()} to PATH to run muxy.")
        print("Reopen Muxy to use the new build. Existing servers and sessions were left running.")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        sys.exit(f"Error: {error}")
