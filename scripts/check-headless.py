#!/usr/bin/env python3
"""Check the native CLI dependency closure without compiling desktop crates."""

import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import slow_test_guard  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent


def packages(metadata):
    workspace = set(metadata["workspace_members"])
    by_name = {p["name"]: p for p in metadata["packages"] if p["id"] in workspace}
    pending = ["muxy-cli", "muxy-server"]
    selected = set()
    while pending:
        name = pending.pop()
        if name in selected:
            continue
        selected.add(name)
        package = by_name[name]
        for dependency in package["dependencies"]:
            dep = dependency["name"]
            if dep in {"muxy-app", "muxy-ui"} or dep.startswith("gpui"):
                raise ValueError(f"{name} brings desktop dependency {dep} into the headless graph")
            if dep in by_name:
                pending.append(dep)
    return sorted(selected)


def run(*command, **kwargs):
    print("+ " + " ".join(map(str, command)), flush=True)
    slow_test_guard.run(command, cwd=ROOT, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--packages", action="store_true", help="print the workspace package closure")
    parser.add_argument("--target", choices=["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"])
    args = parser.parse_args()
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--no-deps", "--all-features", "--format-version=1"], cwd=ROOT))
    selected = packages(metadata)
    if args.packages:
        print("\n".join(selected))
        return
    host = subprocess.check_output(["rustc", "-vV"], text=True)
    expected_arch = {"x86_64-unknown-linux-gnu": "x86_64", "aarch64-unknown-linux-gnu": "aarch64"}.get(args.target)
    if platform.system() != "Linux" or platform.machine() != expected_arch or f"host: {args.target}\n" not in host:
        raise ValueError("Headless verification requires the requested native Linux host and Rust toolchain")
    glibc = subprocess.check_output(["getconf", "GNU_LIBC_VERSION"], text=True).strip()
    if glibc != "glibc 2.35":
        raise ValueError(f"Build in controlled glibc 2.35 userspace, found {glibc}")
    print(f"Native verification: {args.target}, {glibc}; packages: {', '.join(selected)}", flush=True)
    zig = shutil.which("zig")
    if not zig:
        raise ValueError("Install Zig 0.15.2 before running headless verification")
    os.environ["MUXY_ZIG"] = zig
    os.environ["PATH"] = str(ROOT / "scripts/zig") + os.pathsep + os.environ["PATH"]
    # Cargo cannot detect that a cached native Zig library targets another CPU.
    run("cargo", "clean", "-p", "libghostty-vt-sys")
    package_args = [arg for name in selected for arg in ("-p", name)]
    run("cargo", "clippy", "--locked", *package_args, "--all-targets", "--all-features", "--", "-D", "warnings")
    run("cargo", "test", "--locked", *package_args, "--all-features", "--no-fail-fast")
    run("cargo", "build", "--locked", "-p", "muxy-cli", "-p", "muxy-server")
    binary = ROOT / "target/debug/muxy"
    for executable in [binary, binary.with_name("muxy-server")]:
        run(sys.executable, ROOT / "scripts/audit-linux.py", executable, "--target", args.target)
    run(sys.executable, ROOT / "scripts/smoke-headless.py", binary)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
