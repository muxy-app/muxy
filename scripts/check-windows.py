#!/usr/bin/env python3
"""Check the whole workspace, desktop app included, on a native Windows host."""

import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
TARGETS = ["aarch64-pc-windows-msvc", "x86_64-pc-windows-msvc"]
BINARIES = ["muxy-app", "muxy", "muxy-server"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    args = parser.parse_args()
    compiler = subprocess.check_output(["rustc", "-vV"], cwd=ROOT, text=True)
    if platform.system() != "Windows" or f"host: {args.target}\n" not in compiler:
        raise ValueError("The Windows check requires a native Windows host and the requested Rust toolchain")
    if "CARGO_BUILD_TARGET" in os.environ:
        raise ValueError("Remove CARGO_BUILD_TARGET: the check reads the host build's binaries")
    if not shutil.which("zig"):
        raise ValueError("Install Zig 0.15.2 before running the Windows check")
    env = {
        **os.environ,
        "RUSTDOCFLAGS": (os.environ.get("RUSTDOCFLAGS", "") + " -D warnings").strip(),
    }
    commands = [
        [sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests"],
        ["cargo", "fmt", "--all", "--check"],
        ["cargo", "clippy", "--locked", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"],
        ["cargo", "test", "--locked", "--workspace", "--all-features"],
        ["cargo", "doc", "--locked", "--workspace", "--no-deps"],
        ["cargo", "build", "--locked", "-p", "muxy-app", "-p", "muxy-cli", "-p", "muxy-server"],
    ]
    for command in commands:
        print("+ " + " ".join(command), flush=True)
        subprocess.run(command, cwd=ROOT, env=env, check=True)
    target_dir = os.environ.get("CARGO_TARGET_DIR") or os.environ.get("CARGO_BUILD_TARGET_DIR") or "target"
    binaries = ROOT / target_dir / "debug"
    builds = {}
    for name in BINARIES:
        # A pipe makes the GUI-subsystem app write its build info there.
        output = subprocess.check_output([binaries / f"{name}.exe", "--build-info"], cwd=ROOT, timeout=60)
        builds[name] = json.loads(output)
        print(f"{name}: {builds[name]}", flush=True)
    if any(build != builds["muxy-app"] for build in builds.values()):
        raise ValueError("The app, CLI and server report different builds")
    print(f"Windows checks passed on {args.target}; they are headless and replace no human check.", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        sys.exit(str(error))
