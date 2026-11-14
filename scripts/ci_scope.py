"""Decide how much of the test suite a pull request needs.

A pull request that changes only the desktop crates tests only those crates. Any
other change, and any doubt about what changed, tests the whole workspace.
"""

from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
FULL = "full"
DESKTOP = "desktop"
SCOPES = (FULL, DESKTOP)
DESKTOP_CRATES = ("muxy-app", "muxy-ui")
DESKTOP_TEST = [
    "cargo", "test", "--locked", *(arg for crate in DESKTOP_CRATES for arg in ("-p", crate)),
    "--all-features", "--timings",
]


def classify(paths):
    """Return DESKTOP only for a non-empty set of desktop crate sources, FULL for anything else."""
    prefixes = tuple(f"crates/{crate}/" for crate in DESKTOP_CRATES)
    if paths and all(path.startswith(prefixes) and not path.endswith("/Cargo.toml") for path in paths):
        return DESKTOP
    return FULL


def tests(scope, full):
    """Return the test commands for a scope, given the ones that cover the whole workspace."""
    return [DESKTOP_TEST] if scope == DESKTOP else full


def git(*args, cwd):
    return subprocess.check_output(["git", *args], cwd=cwd, text=True, errors="surrogateescape")


def changed_paths(cwd=ROOT):
    """Return what HEAD, a pull request's test merge, changes on its base; empty if that is unknowable.

    Comparing without rename detection lists a file moved between crates at both
    of its paths, so moving one into a desktop crate still counts as changing the
    crate it left.
    """
    try:
        parents = git("rev-list", "--parents", "--max-count=1", "HEAD", cwd=cwd).split()[1:]
        if len(parents) != 2:
            print(f"HEAD has {len(parents)} parents, not a test merge's 2", file=sys.stderr)
            return []
        names = git("diff", "--no-renames", "--name-only", "-z", parents[0], "HEAD", cwd=cwd)
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"Could not read the change: {error}", file=sys.stderr)
        return []
    return [name for name in names.split("\0") if name]


def main():
    paths = changed_paths()
    scope = classify(paths)
    print(f"{len(paths)} changed files; testing scope: {scope}", file=sys.stderr)
    print(f"scope={scope}")


if __name__ == "__main__":
    main()
