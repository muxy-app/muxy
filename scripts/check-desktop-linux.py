#!/usr/bin/env python3
"""Check the whole workspace, desktop app included, on a native Linux desktop userspace."""

import argparse
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import ci_scope  # noqa: E402
import slow_test_guard  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
TARGETS = {"x86_64-unknown-linux-gnu": "x86_64", "aarch64-unknown-linux-gnu": "aarch64"}
PARTS = {
    "lint": [
        [sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests"],
        ["cargo", "fmt", "--all", "--check"],
        ["cargo", "clippy", "--locked", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"],
        ["cargo", "doc", "--locked", "--workspace", "--no-deps"],
    ],
    "test": [
        ["cargo", "test", "--locked", "--workspace", "--all-features", "--timings"],
    ],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=sorted(TARGETS), required=True)
    parser.add_argument("--part", choices=sorted(PARTS), help="run only this part; CI runs the parts in parallel")
    parser.add_argument("--scope", choices=ci_scope.SCOPES, default=ci_scope.FULL, help="which crates to test")
    args = parser.parse_args()
    compiler = subprocess.check_output(["rustc", "-vV"], cwd=ROOT, text=True)
    if (platform.system() != "Linux" or platform.machine() != TARGETS[args.target]
            or f"host: {args.target}\n" not in compiler):
        raise ValueError("The Linux desktop check requires the requested native Linux host and Rust toolchain")
    if os.environ.get("CARGO_BUILD_TARGET", args.target) != args.target:
        raise ValueError("Remove CARGO_BUILD_TARGET or set it to the native Linux host")
    wrapper = ROOT / "scripts/zig"
    zig = os.environ.get("MUXY_ZIG") or shutil.which("zig")
    if not zig or Path(zig).resolve() == (wrapper / "zig").resolve():
        raise ValueError("Install Zig 0.17.0 or set MUXY_ZIG to it before running the Linux desktop check")
    env = {
        **os.environ,
        "MUXY_ZIG": zig,
        "PATH": str(wrapper) + os.pathsep + os.environ.get("PATH", ""),
        "RUSTDOCFLAGS": (os.environ.get("RUSTDOCFLAGS", "") + " -D warnings").strip(),
    }
    parts = {**PARTS, "test": ci_scope.tests(args.scope, PARTS["test"])}
    for command in parts[args.part] if args.part else [*parts["lint"], *parts["test"]]:
        print("+ " + " ".join(command), flush=True)
        slow_test_guard.run(command, cwd=ROOT, env=env)
    print(f"Linux desktop checks passed on {args.target}; they are headless and replace no human check.", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
