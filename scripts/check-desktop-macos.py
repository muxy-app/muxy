import argparse
import os
from pathlib import Path
import platform
import resource
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import slow_test_guard  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
PARTS = {
    "lint": [
        [sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests"],
        ["cargo", "fmt", "--all", "--check"],
        ["cargo", "clippy", "--locked", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"],
        ["cargo", "doc", "--locked", "--workspace", "--no-deps"],
    ],
    "test": [
        ["cargo", "test", "--locked", "--workspace", "--all-features"],
    ],
}


def main():
    parser = argparse.ArgumentParser(description="Run the native macOS desktop regression guard.")
    parser.add_argument("--part", choices=sorted(PARTS), help="run only this part; CI runs the parts in parallel")
    args = parser.parse_args()
    if platform.system() != "Darwin":
        raise ValueError("The macOS guard must run on macOS; Linux checks do not replace it")
    compiler = subprocess.check_output(["rustc", "-vV"], cwd=ROOT, text=True)
    host = next((line.removeprefix("host: ") for line in compiler.splitlines() if line.startswith("host: ")), "")
    if host not in {"aarch64-apple-darwin", "x86_64-apple-darwin"}:
        raise ValueError("The macOS guard requires a native macOS Rust toolchain")
    if os.environ.get("CARGO_BUILD_TARGET", host) != host:
        raise ValueError("Remove CARGO_BUILD_TARGET or set it to the native macOS host")
    soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    requested = 10240 if hard == resource.RLIM_INFINITY else min(10240, hard)
    if soft != resource.RLIM_INFINITY and soft < requested:
        resource.setrlimit(resource.RLIMIT_NOFILE, (requested, hard))
    env = {
        **os.environ,
        "RUSTDOCFLAGS": (os.environ.get("RUSTDOCFLAGS", "") + " -D warnings").strip(),
        "RUST_TEST_THREADS": "4",
    }
    for command in PARTS[args.part] if args.part else [*PARTS["lint"], *PARTS["test"]]:
        print("+ " + " ".join(command), flush=True)
        slow_test_guard.run(command, cwd=ROOT, env=env)
    print(f"macOS automated guard passed on {host}; record required human checks separately.", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
