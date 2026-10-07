#!/usr/bin/env python3
"""Announce a published beta or stable release on Discord with its release notes."""

import argparse
import json
import os
import re
import subprocess
import sys
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from release import channel  # noqa: E402

ANNOUNCEMENTS = {
    "beta": ("Muxy {version} (beta) released", "Beta channel", 15844367),
    "stable": ("Muxy v{version} released", "Stable channel", 3066993),
}
NOTES_LIMIT = 3800


def payload(tag, notes, repository, now):
    if not tag.startswith("v"):
        raise ValueError("release tag must be vX.Y.Z or vX.Y.Z-beta.N")
    version = tag[1:]
    title, footer, color = ANNOUNCEMENTS[channel(version)]
    notes = re.sub(r" by @[A-Za-z0-9_-]+ in https://github\.com/\S+/pull/[0-9]+", "", notes)
    if len(notes) > NOTES_LIMIT:
        notes = notes[:NOTES_LIMIT] + "…"
    return {
        "username": "Muxy Releases",
        "embeds": [{
            "title": title.format(version=version),
            "url": f"https://github.com/{repository}/releases/tag/{tag}",
            "description": notes,
            "color": color,
            "footer": {"text": footer},
            "timestamp": now.strftime("%Y-%m-%dT%H:%M:%SZ"),
        }],
    }


def published_notes(tag, repository):
    release = json.loads(subprocess.check_output(
        ["gh", "release", "view", tag, "--repo", repository, "--json", "body,isDraft"], text=True))
    if release["isDraft"]:
        raise ValueError(f"{tag} is a draft; publish it before announcing it")
    return release["body"]


def announce(tag, repository, webhook):
    body = payload(tag, published_notes(tag, repository), repository, datetime.now(timezone.utc))
    request = urllib.request.Request(
        webhook, data=json.dumps(body).encode(), method="POST",
        # Discord rejects requests without a bot-style user agent.
        headers={"Content-Type": "application/json",
                 "User-Agent": f"DiscordBot (https://github.com/{repository}, 1)"},
    )
    with urllib.request.urlopen(request, timeout=30):
        pass


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag", help="vX.Y.Z or vX.Y.Z-beta.N")
    args = parser.parse_args()
    webhook = os.environ.get("DISCORD_WEBHOOK_URL", "")
    repository = os.environ.get("GITHUB_REPOSITORY", "muxy-app/muxy")
    try:
        if not webhook:
            raise ValueError("DISCORD_WEBHOOK_URL is required")
        announce(args.tag, repository, webhook)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"error: could not announce {args.tag} on Discord: {error}\n")
    print(f"Announced {args.tag} on Discord")


if __name__ == "__main__":
    main()
