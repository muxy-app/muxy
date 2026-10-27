import fcntl
import gzip
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]


def module(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / (name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


INSTALL = module("install-desktop-linux")
AUDIT = module("audit-linux")
RELEASE = module("release")


class DesktopDistributionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name)
        self.root = self.home / "desktop with spaces"
        self.root.mkdir(mode=0o700)
        (self.root / "versions").mkdir(mode=0o700)
        INSTALL.atomic_json(self.root / "installation.json", {"schema": 1, "channel": "beta"})
        self.old = "2.0.0-beta.10"
        self.new = "2.0.0-beta.11"
        home = patch.object(Path, "home", return_value=self.home)
        home.start()
        self.addCleanup(home.stop)
        environment = patch.dict(os.environ, {"XDG_DATA_HOME": str(self.home / "data")})
        environment.start()
        self.addCleanup(environment.stop)
        architecture = patch.object(INSTALL, "architecture", return_value="arm64")
        architecture.start()
        self.addCleanup(architecture.stop)

    def package(self, version, **options):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            build = {"version": version, "compatibility": 1, "protocol": [1]}
            files = {}
            for name in INSTALL.FILES - {"manifest.json"}:
                path = directory / name
                value = ("#!/bin/sh\nprintf '%s\\n' '" + json.dumps(build) + "'\n").encode() if name in INSTALL.BINARIES else b"asset"
                path.write_bytes(value)
                path.chmod(0o755 if name in INSTALL.BINARIES else 0o644)
                files[name] = {"size": len(value), "sha256": hashlib.sha256(value).hexdigest()}
            manifest = {"schema": 1, "version": version, "channel": "beta", "arch": INSTALL.architecture(),
                        "build": build, "files": files, **options}
            (directory / "manifest.json").write_text(json.dumps(manifest))
            archive = self.home / f"{version}.tar.gz"
            with tarfile.open(archive, "w:gz") as output:
                for path in directory.iterdir():
                    output.add(path, arcname=f"muxy-desktop/{path.name}")
        return archive, INSTALL.digest(archive)

    def staged(self, version):
        archive, checksum = self.package(version)
        INSTALL.stage(self.root, archive, version, checksum)
        return self.root / "versions" / version

    def test_tar_rejects_traversal_links_duplicate_files_and_oversized_entries(self):
        for name, kind, size in (("../escaped", tarfile.REGTYPE, 1),
                                 ("muxy-desktop/muxy", tarfile.SYMTYPE, 0),
                                 ("muxy-desktop/muxy", tarfile.LNKTYPE, 0),
                                 ("muxy-desktop/muxy", tarfile.FIFOTYPE, 0),
                                 ("muxy-desktop/muxy", tarfile.REGTYPE, INSTALL.MAX_ARCHIVE + 1)):
            with self.subTest(name=name, kind=kind, size=size):
                archive = self.home / "bad.tar.gz"
                with gzip.open(archive, "wb") as output:
                    entry = tarfile.TarInfo(name)
                    entry.type, entry.size, entry.linkname = kind, size, "/tmp/escaped"
                    output.write(entry.tobuf())
                with tempfile.TemporaryDirectory() as temporary, self.assertRaisesRegex(
                        ValueError, "Unsafe or unexpected desktop archive entry"):
                    INSTALL.extract(archive, Path(temporary), INSTALL.digest(archive))
        archive = self.home / "duplicates.tar.gz"
        with tarfile.open(archive, "w:gz") as output:
            for _ in range(2):
                entry = tarfile.TarInfo("muxy-desktop/muxy")
                entry.size = 1
                output.addfile(entry, io.BytesIO(b"x"))
        with tempfile.TemporaryDirectory() as temporary, self.assertRaises(ValueError):
            INSTALL.extract(archive, Path(temporary), INSTALL.digest(archive))
        self.assertFalse((self.home / "escaped").exists())

    def test_wrong_architecture_channel_and_mismatched_helpers_are_rejected(self):
        for options in ({"arch": "unsupported"}, {"channel": "stable"},
                        {"build": {"version": self.new, "compatibility": 999, "protocol": [99]}}):
            with self.subTest(options=options):
                archive, checksum = self.package(self.new, **options)
                with self.assertRaises(ValueError):
                    INSTALL.stage(self.root, archive, self.new, checksum)
                self.assertFalse((self.root / "versions" / self.new).exists())

    def test_install_and_removal_preserve_unrelated_commands_and_user_data(self):
        self.staged(self.old)
        INSTALL.switch(self.root, self.old)
        commands = self.home / ".local/bin"
        commands.mkdir(parents=True)
        unrelated = commands / "muxy-beta"
        unrelated.write_text("unrelated CLI")
        profile = self.home / "data/muxy-beta"
        profile.mkdir(parents=True)
        (profile / "settings.toml").write_text("keep me")
        INSTALL.integrate(self.root, self.old)
        self.assertEqual(unrelated.read_text(), "unrelated CLI")
        with (self.root / "versions" / self.old / ".lease").open() as running:
            fcntl.flock(running, fcntl.LOCK_SH)
            with self.assertRaises(BlockingIOError):
                INSTALL.uninstall(self.root)
            self.assertTrue((commands / "muxy-server-beta").exists())
        INSTALL.uninstall(self.root)
        self.assertFalse(self.root.exists())
        self.assertFalse((commands / "muxy-server-beta").is_symlink())
        self.assertEqual(unrelated.read_text(), "unrelated CLI")
        self.assertEqual((profile / "settings.toml").read_text(), "keep me")

    def test_appimage_updates_its_launcher_without_overwriting_other_formats_or_channels(self):
        store = self.home / "data/muxy-appimage"
        old, new = store / ("a" * 64), store / ("b" * 64)
        old.mkdir(parents=True)
        new.mkdir()
        INSTALL.integrate_appimage(old, self.old)
        entry, old_contents = INSTALL.desktop_entry(old, self.old, appimage=True)
        self.assertEqual(entry.read_text(), old_contents)
        INSTALL.integrate_appimage(new, self.new)
        self.assertEqual(entry.read_text(), INSTALL.desktop_entry(new, self.new, appimage=True)[1])
        self.assertIn(f'Exec="{new}/usr/bin/muxy-app" %F', entry.read_text())
        self.assertIn(f"Icon={new}/com.muxy-beta.app.png", entry.read_text())
        INSTALL.integrate_appimage(new, "2.2.0")
        stable, stable_contents = INSTALL.desktop_entry(new, "2.2.0", appimage=True)
        self.assertEqual(stable.read_text(), stable_contents)
        for unrelated in (old_contents + "X-User-Customized=true\n", INSTALL.desktop_entry(self.root, self.old)[1]):
            entry.write_text(unrelated)
            INSTALL.integrate_appimage(new, self.new)
            self.assertEqual(entry.read_text(), unrelated)
            self.assertEqual(stable.read_text(), stable_contents)
        entry.unlink()
        entry.symlink_to(stable)
        INSTALL.integrate_appimage(new, self.new)
        self.assertTrue(entry.is_symlink())
        self.assertEqual(stable.read_text(), stable_contents)

    def test_symlinked_installation_and_competing_mutations_are_rejected(self):
        link = self.home / "linked"
        link.symlink_to(self.root)
        with self.assertRaises(ValueError), INSTALL.locked(link):
            pass
        with INSTALL.locked(self.root):
            with self.assertRaises(BlockingIOError), INSTALL.locked(self.root):
                pass

    def test_uninstall_keeps_unowned_files_inside_the_installation_root(self):
        self.staged(self.old)
        INSTALL.switch(self.root, self.old)
        retained = self.root / "notes.txt"
        retained.write_text("user data")
        INSTALL.uninstall(self.root)
        self.assertEqual(retained.read_text(), "user data")
        self.assertFalse((self.root / "versions").exists())

    def test_downloads_refuse_a_plaintext_redirect_before_following_it(self):
        request = INSTALL.urllib.request.Request("https://github.com/muxy-app/muxy/releases/download/test")
        with self.assertRaisesRegex(ValueError, "insecure"):
            INSTALL.HTTPSRedirectHandler().redirect_request(request, None, 302, "redirect", {}, "http://example.com/archive")

    def test_latest_install_uses_existing_feeds_without_switching_channels_or_selecting_cli_only_releases(self):
        def feed(version, platforms):
            return json.dumps({"schema": 1, "version": version, "platforms": platforms}).encode()
        for selected, version, url in (
                ("stable", "2.2.0", "https://github.com/muxy-app/muxy/releases/latest/download/update.json"),
                ("beta", self.new, f"{INSTALL.RELEASES}/beta-2.x/update.json")):
            def fetch(actual_url, output, limit):
                self.assertEqual(actual_url, url)
                output.write(feed(version, {"linux-aarch64": {}}))
            with patch.object(INSTALL, "fetch", side_effect=fetch):
                self.assertEqual(INSTALL.latest(selected), version)
        for data, reason in ((feed("2.2.0", {"macos-aarch64": {}}), "no Linux desktop"),
                             (feed(self.new, {"linux-aarch64": {}}), "channel metadata")):
            with patch.object(INSTALL, "fetch", side_effect=lambda url, output, limit: output.write(data)):
                with self.assertRaisesRegex(ValueError, reason):
                    INSTALL.latest("stable")

    def test_linux_metadata_is_complete_checksummed_and_keeps_macos_assets(self):
        for name in (f"Muxy-{self.new}-arm64.dmg", f"Muxy-{self.new}-x86_64.dmg",
                     f"muxy-desktop-{self.new}-linux-arm64.tar.gz", f"muxy-desktop-{self.new}-linux-x86_64.tar.gz"):
            (self.home / name).write_bytes(b"release bytes")
        metadata = RELEASE.update_metadata(self.new, "muxy-app/muxy", self.home)
        self.assertEqual(set(metadata["platforms"]), {"macos-aarch64", "macos-x86_64", "linux-aarch64", "linux-x86_64"})
        self.assertEqual(metadata["platforms"]["linux-aarch64"]["sha256"], hashlib.sha256(b"release bytes").hexdigest())
        (self.home / f"muxy-desktop-{self.new}-linux-x86_64.tar.gz").unlink()
        with self.assertRaises(FileNotFoundError):
            RELEASE.update_metadata(self.new, "muxy-app/muxy", self.home)

    def test_desktop_audit_does_not_weaken_standalone_glibc_or_library_policy(self):
        inputs = ["Class: ELF64\nData: little endian\nMachine: AArch64\n",
                  "[Requesting program interpreter: /lib/ld-linux-aarch64.so.1]",
                  "(NEEDED) [libc.so.6]\n(NEEDED) [libfreetype.so.6]\n",
                  "Name: GLIBC_2.39\n", "libc.so.6 => /lib/aarch64-linux-gnu/libc.so.6", "aarch64-unknown-linux-gnu"]
        AUDIT.audit(*inputs, desktop=True)
        with self.assertRaises(ValueError):
            AUDIT.audit(*inputs)
        for index, addition in ((2, "(NEEDED) [libunknown.so]\n"), (3, "Name: GLIBC_2.40\n"),
                                (4, "\nlibfreetype.so.6 => not found"), (2, "(RUNPATH) [/build]")):
            bad = inputs.copy()
            bad[index] += addition
            with self.assertRaises(ValueError):
                AUDIT.audit(*bad, desktop=True)


if __name__ == "__main__":
    unittest.main()
