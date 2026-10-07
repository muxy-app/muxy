#!/usr/bin/env python3
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

from release import build_number

CHANNEL = "beta-2.x"
# Feeds published before BETA_VERSION numbering hold 2.0.0-beta-N and are always older.
LEGACY_VERSION = re.compile(r"2\.0\.0-beta-[1-9][0-9]*")


def gh(repository, *args, check=True):
    return subprocess.run(
        ["gh", "release", *args, "--repo", repository],
        capture_output=True, text=True, check=check,
    )


def download(repository, tag, directory):
    gh(repository, "download", tag, "--pattern", "update.json", "--dir", str(directory))
    return json.loads((directory / "update.json").read_text())


def publish(version, repository, sha):
    incoming = int(build_number(version))
    with tempfile.TemporaryDirectory(prefix="muxy-beta-feed-") as temporary:
        directory = Path(temporary)
        candidate = directory / "candidate"
        candidate.mkdir()
        metadata = download(repository, f"v{version}", candidate)
        if metadata.get("schema") != 1 or metadata.get("version") != version:
            raise ValueError("published release has mismatched update metadata")
        response = gh(repository, "view", CHANNEL, "--json", "isDraft,isPrerelease,assets", check=False)
        if response.returncode == 0:
            channel = json.loads(response.stdout)
            if not channel["isPrerelease"]:
                raise ValueError("refusing to modify a stable release")
            if any(asset["name"] == "update.json" for asset in channel["assets"]):
                current = directory / "current"
                current.mkdir()
                previous = download(repository, CHANNEL, current)
                if previous.get("schema") != 1:
                    raise ValueError("unsupported existing update metadata")
                if (not LEGACY_VERSION.fullmatch(previous["version"])
                        and int(build_number(previous["version"])) > incoming):
                    print("The beta feed already points to a newer build; leaving it unchanged")
                    return
        else:
            gh(repository, "create", CHANNEL, "--target", sha, "--title", "Muxy 2.x Beta Updates",
               "--draft", "--prerelease", "--latest=false", "--notes",
               "Update feed for Muxy 2.x beta. Downloads are attached to each versioned beta release.")
        gh(repository, "upload", CHANNEL, str(candidate / "update.json"), "--clobber")
        gh(repository, "edit", CHANNEL, "--draft=false", "--prerelease", "--latest=false")


if __name__ == "__main__":
    try:
        publish(sys.argv[1], os.environ["GITHUB_REPOSITORY"], os.environ["GITHUB_SHA"])
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        detail = error.stderr if isinstance(error, subprocess.CalledProcessError) else str(error)
        sys.exit(f"Could not publish the beta update feed: {detail}")
