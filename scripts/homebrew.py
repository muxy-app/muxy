#!/usr/bin/env python3
"""Write the Homebrew cask (desktop app) and formula (CLI and server) of a stable release.

Both pin the release's assets by the hashes in its SHA256SUMS.
"""

import argparse
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from release import channel  # noqa: E402


def checksums(path):
    sums = {}
    for line in path.read_text().splitlines():
        digest, name = line.split(maxsplit=1)
        if not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise ValueError(f"invalid checksum line: {line}")
        sums[name.lstrip("*")] = digest
    return sums


def digest(sums, name):
    if name not in sums:
        raise ValueError(f"SHA256SUMS has no {name}")
    return sums[name]


def cask(version, sums, repository):
    arm, intel = (digest(sums, f"Muxy-{version}-{arch}.dmg") for arch in ("arm64", "x86_64"))
    # The app updates itself and keeps running servers through it; brew upgrade would not.
    return f'''cask "muxy" do
  arch arm: "arm64", intel: "x86_64"

  version "{version}"
  sha256 arm:   "{arm}",
         intel: "{intel}"

  url "https://github.com/{repository}/releases/download/v#{{version}}/Muxy-#{{version}}-#{{arch}}.dmg"
  name "Muxy"
  desc "Terminal multiplexer"
  homepage "https://github.com/{repository}"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true
  depends_on macos: :sonoma

  app "Muxy.app"

  zap trash: [
    "~/Library/Application Support/Muxy 2",
    "~/Library/Application Support/Muxy",
    "~/Library/Caches/com.muxy.app",
    "~/Library/HTTPStorages/com.muxy.app",
    "~/Library/HTTPStorages/com.muxy.app.binarycookies",
    "~/Library/Logs/Muxy",
    "~/Library/Preferences/com.muxy.app.plist",
    "~/Library/Saved Application State/com.muxy.app.savedState",
    "~/Library/WebKit/com.muxy.app",
  ]
end
'''


def formula(version, sums, repository):
    def source(platform, arch, extension):
        name = f"muxy-{version}-{platform}-{arch}.{extension}"
        return (f'      url "https://github.com/{repository}/releases/download/v{version}/{name}"\n'
                f'      sha256 "{digest(sums, name)}"')

    return f'''class MuxyCli < Formula
  desc "Terminal multiplexer: the muxy CLI, terminal UI, and server"
  homepage "https://github.com/{repository}"
  version "{version}"
  license "MIT"

  on_macos do
    depends_on macos: :sonoma

    on_arm do
{source("macos", "arm64", "zip")}
    end
    on_intel do
{source("macos", "x86_64", "zip")}
    end
  end

  on_linux do
    on_arm do
{source("linux", "arm64", "tar.gz")}
    end
    on_intel do
{source("linux", "x86_64", "tar.gz")}
    end
  end

  def install
    bin.install "muxy", "muxy-server"
  end

  test do
    assert_match "\\"version\\":\\"#{{version}}\\"", shell_output("#{{bin}}/muxy --build-info")
    assert_match "\\"version\\":\\"#{{version}}\\"", shell_output("#{{bin}}/muxy-server --build-info")
  end
end
'''


def write(version, sums_path, tap, repository):
    if channel(version) != "stable":
        raise ValueError("Homebrew installs stable releases only")
    sums = checksums(sums_path)
    files = {
        tap / "Casks/muxy.rb": cask(version, sums, repository),
        tap / "Formula/muxy-cli.rb": formula(version, sums, repository),
    }
    for path, text in files.items():
        path.parent.mkdir(exist_ok=True)
        path.write_text(text)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    parser.add_argument("checksums", type=Path, help="the release's SHA256SUMS")
    parser.add_argument("tap", type=Path, help="a checkout of the Homebrew tap")
    parser.add_argument("--repository", default="muxy-app/muxy")
    args = parser.parse_args()
    try:
        write(args.version, args.checksums, args.tap, args.repository)
    except (ValueError, OSError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
