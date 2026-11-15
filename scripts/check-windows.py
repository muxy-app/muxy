#!/usr/bin/env python3
"""Check the whole workspace, desktop app included, on a native Windows host."""

import argparse
from pathlib import Path
import platform
import shutil
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import ci_scope  # noqa: E402
import slow_test_guard  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
TARGETS = ["aarch64-pc-windows-msvc", "x86_64-pc-windows-msvc"]
PARTS = {
    "lint": [
        [sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests"],
        ["cargo", "clippy", "--locked", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"],
    ],
    "test": [
        ["cargo", "test", "--locked", "--workspace", "--all-features", "--tests", "--timings"],
    ],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--part", choices=sorted(PARTS), help="run only this part; CI runs the parts in parallel")
    parser.add_argument("--scope", choices=ci_scope.SCOPES, default=ci_scope.FULL, help="which crates to test")
    args = parser.parse_args()
    compiler = subprocess.check_output(["rustc", "-vV"], cwd=ROOT, text=True)
    if platform.system() != "Windows" or f"host: {args.target}\n" not in compiler:
        raise ValueError("The Windows check requires a native Windows host and the requested Rust toolchain")
    if not shutil.which("zig"):
        raise ValueError("Install Zig 0.17.0 before running the Windows check")
    parts = {**PARTS, "test": ci_scope.tests(args.scope, PARTS["test"])}
    for command in parts[args.part] if args.part else [*parts["lint"], *parts["test"]]:
        print("+ " + " ".join(command), flush=True)
        slow_test_guard.run(command, cwd=ROOT)
    print(f"Windows checks passed on {args.target}; they are headless and replace no human check.", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        sys.exit(str(error))
