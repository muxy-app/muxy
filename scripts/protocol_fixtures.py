#!/usr/bin/env python3
"""Keep protocol fixtures honest across builds.

Fixtures record what builds of the current protocol version send, so within a
version they may only be added. The last release must also read every fixture
this build writes.
"""
import argparse
import os
import re
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = "crates/muxy-protocol/tests/fixtures"
VERSION = "crates/muxy-protocol/src/version.rs"
FIXTURE = re.compile(rf"^{FIXTURES}/v\d+/[^/]+\.bin$")


def git(*args):
    return subprocess.run(
        ["git", "-C", str(ROOT), *args], check=True, capture_output=True, text=True
    ).stdout


def rewritten(name_status):
    """Fixtures a diff modified, deleted, or renamed; additions are allowed."""
    paths = []
    for line in name_status.splitlines():
        status, path, *_ = line.split("\t")
        if status != "A" and FIXTURE.match(path):
            paths.append(path)
    return paths


def current_version(text):
    match = re.search(r"^pub const CURRENT: Version = V(\d+);", text, re.MULTILINE)
    return int(match[1]) if match else None


def append_only(base):
    if not base or set(base) == {"0"}:
        return
    changed = rewritten(git("diff", "--name-status", base, "--", FIXTURES))
    if not changed:
        return
    try:
        before = current_version(git("show", f"{base}:{VERSION}"))
    except subprocess.CalledProcessError:
        before = None
    if before != current_version((ROOT / VERSION).read_text()):
        return
    raise SystemExit(
        "Protocol fixtures are what older builds send, so they are append-only within a "
        "protocol version. Changed: " + ", ".join(changed) + ". Keep them, or bump CURRENT "
        "in crates/muxy-protocol/src/version.rs for a breaking change."
    )


def last_release(version):
    """The newest beta tag, if it speaks `version`; otherwise the version is unreleased."""
    tags = git("tag", "--list", "v2.0.0-beta-*").split()
    if not tags:
        return None
    tag = max(tags, key=lambda tag: int(tag.rsplit("-", 1)[1]))
    if git("ls-tree", "-d", "--name-only", tag, f"{FIXTURES}/v{version}").strip():
        return tag
    return None


def read_by_last_release():
    version = current_version((ROOT / VERSION).read_text())
    tag = last_release(version)
    if tag is None:
        print(f"No release has v{version} fixtures yet; skipping the cross-version check.")
        return
    # A fixed path lets repeated runs reuse the build cache.
    checkout = ROOT / "target" / "last-release"
    shutil.rmtree(checkout, ignore_errors=True)
    git("worktree", "prune")
    git("worktree", "add", "--detach", str(checkout), tag)
    try:
        subprocess.run(
            ["cargo", "test", "--locked", "-p", "muxy-protocol", "--test", "fixtures",
             "every_fixture_decodes", "--", "--exact"],
            cwd=checkout,
            check=True,
            env={
                **os.environ,
                "CARGO_TARGET_DIR": str(ROOT / "target"),
                "MUXY_PROTOCOL_FIXTURES": str(ROOT / FIXTURES / f"v{version}"),
            },
        )
    finally:
        git("worktree", "remove", "--force", str(checkout))
    print(f"{tag} reads every v{version} fixture of this build.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    guard = commands.add_parser("append-only", help="fail if a fixture changed without a version bump")
    guard.add_argument("--base", default="")
    commands.add_parser("last-release", help="check the last release reads this build's fixtures")
    args = parser.parse_args()
    if args.command == "append-only":
        append_only(args.base)
    else:
        read_by_last_release()


if __name__ == "__main__":
    main()
