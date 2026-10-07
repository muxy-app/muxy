#!/usr/bin/env python3
"""Sign a stable DMG with Sparkle and write the appcast that updates Muxy 1.x to it.

Muxy 1.x checks the latest release's appcast with Sparkle. Signing with its EdDSA
key and keeping its bundle identifier lets it install Muxy 2 in place.
"""

import argparse
import base64
import hashlib
import os
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
from datetime import datetime, timezone
from email.utils import format_datetime
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from release import channel  # noqa: E402

SPARKLE = "https://github.com/sparkle-project/Sparkle/releases/download/2.9.1/Sparkle-2.9.1.tar.xz"
SPARKLE_SHA256 = "c0dde519fd2a43ddfc6a1eb76aec284d7d888fe281414f9177de3164d98ba4c7"


def download_sign_update(directory):
    """Sparkle's sign_update from the pinned release archive."""
    archive = directory / "Sparkle.tar.xz"
    with urllib.request.urlopen(SPARKLE, timeout=300) as response:
        archive.write_bytes(response.read())
    if hashlib.sha256(archive.read_bytes()).hexdigest() != SPARKLE_SHA256:
        raise ValueError("Sparkle archive does not match its pinned checksum")
    tool = directory / "sign_update"
    with tarfile.open(archive) as bundle:
        tool.write_bytes(bundle.extractfile(bundle.getmember("./bin/sign_update")).read())
    tool.chmod(0o755)
    return tool


def sign(tool, dmg, key):
    """The EdDSA signature of `dmg`, checked by Sparkle before it is published."""
    def run(*args):
        return subprocess.run([str(tool), "--ed-key-file", "-", *args], input=key,
                              capture_output=True, text=True, check=True).stdout.strip()

    signature = run("-p", str(dmg))
    try:
        valid = len(base64.b64decode(signature, validate=True)) == 64
    except ValueError:
        valid = False
    if not valid:
        raise ValueError("sign_update did not print an EdDSA signature")
    run("--verify", str(dmg), signature)
    return signature


def appcast(version, build, repository, dmg, signature):
    published = format_datetime(datetime.now(timezone.utc))
    return f"""<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <title>Muxy Updates</title>
    <link>https://github.com/{repository}</link>
    <description>Muxy stable releases</description>
    <language>en</language>
    <item>
      <title>Version {version}</title>
      <pubDate>{published}</pubDate>
      <sparkle:version>{build}</sparkle:version>
      <sparkle:shortVersionString>{version}</sparkle:shortVersionString>
      <sparkle:minimumSystemVersion>14.0</sparkle:minimumSystemVersion>
      <sparkle:fullReleaseNotesLink>https://github.com/{repository}/releases/tag/v{version}</sparkle:fullReleaseNotesLink>
      <enclosure url="https://github.com/{repository}/releases/download/v{version}/{dmg.name}" sparkle:edSignature="{signature}" length="{dmg.stat().st_size}" type="application/octet-stream" />
    </item>
  </channel>
</rss>
"""


def write(args):
    if channel(args.version) != "stable":
        raise ValueError("only stable releases update Muxy 1.x")
    if not args.build.isdigit() or args.build.startswith("0"):
        raise ValueError("build number must be a positive commit count")
    if args.dmg.name not in {f"Muxy-{args.version}-{arch}.dmg" for arch in ("arm64", "x86_64")}:
        raise ValueError("expected a Muxy-<version>-<arch>.dmg")
    key = os.environ.get("SPARKLE_PRIVATE_KEY", "")
    if not key:
        raise ValueError("SPARKLE_PRIVATE_KEY is required")
    with tempfile.TemporaryDirectory(prefix="muxy-sparkle-") as temporary:
        tool = args.sign_update or download_sign_update(Path(temporary))
        signature = sign(tool, args.dmg, key)
    args.output.write_text(appcast(args.version, args.build, args.repository, args.dmg, signature))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    parser.add_argument("build", help="the commit count, which Sparkle compares with 1.x's")
    parser.add_argument("dmg", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY", "muxy-app/muxy"))
    parser.add_argument("--sign-update", type=Path, help="an existing sign_update instead of the pinned download")
    args = parser.parse_args()
    try:
        write(args)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        detail = error.stderr or error.stdout if isinstance(error, subprocess.CalledProcessError) else error
        parser.exit(1, f"error: {detail}\n")
    print(f"Wrote {args.output} for {args.dmg.name}")


if __name__ == "__main__":
    main()
