import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("build_install", ROOT / "scripts/build-install.py")
build_install = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(build_install)


class BuildInstallTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="muxy local tests ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.artifact = self.root / "built"
        self.artifact.mkdir()
        self.bin_dir = self.root / "commands"
        for name in build_install.BINARIES:
            binary = self.artifact / name
            binary.write_text("#!/bin/sh\necho new-" + name + "\n")
            binary.chmod(0o755)

    def test_linux_install_and_reinstall_replace_both_commands(self):
        build_install.install(self.artifact, self.bin_dir)
        for name in build_install.BINARIES:
            self.assertEqual(subprocess.check_output([self.bin_dir / name], text=True).strip(), "new-" + name)
            self.assertEqual((self.bin_dir / name).resolve().parent, (self.bin_dir / ".muxy-local").resolve())
        (self.artifact / "muxy").write_text("updated")
        build_install.install(self.artifact, self.bin_dir)
        self.assertEqual((self.bin_dir / "muxy").read_text(), "updated")
        self.assertFalse(list(self.bin_dir.glob(".muxy-install-*")))

    def test_failed_link_replacement_restores_old_installation_and_links(self):
        build_install.install(self.artifact, self.bin_dir)
        old_link = self.bin_dir / "muxy-server"
        old_link.unlink()
        old_link.symlink_to("missing-old-server")
        (self.artifact / "muxy").write_text("updated")
        replace = os.replace

        def fail_second_link(source, destination):
            if Path(source).name == "muxy-server":
                raise OSError("injected link failure")
            replace(source, destination)

        with patch.object(build_install.os, "replace", side_effect=fail_second_link):
            with self.assertRaisesRegex(OSError, "injected link failure"):
                build_install.install(self.artifact, self.bin_dir)
        self.assertIn("new-muxy", (self.bin_dir / "muxy").read_text())
        self.assertEqual(os.readlink(old_link), "missing-old-server")

    def test_native_targets_and_linux_glibc_floor(self):
        for system, arch, target in (
            ("Darwin", "arm64", "aarch64-apple-darwin"),
            ("Darwin", "x86_64", "x86_64-apple-darwin"),
            ("Linux", "aarch64", "aarch64-unknown-linux-gnu"),
            ("Linux", "x86_64", "x86_64-unknown-linux-gnu"),
        ):
            with self.subTest(system=system, arch=arch), \
                    patch.object(build_install.platform, "system", return_value=system), \
                    patch.object(build_install.platform, "machine", return_value=arch), \
                    patch.object(build_install.platform, "mac_ver", return_value=("14.0", (), "")), \
                    patch.object(build_install, "output", return_value="glibc 2.39"):
                self.assertEqual(build_install.host_target(), (system, target))
        with patch.object(build_install.platform, "system", return_value="Linux"), \
                patch.object(build_install.platform, "machine", return_value="aarch64"):
            for libc in ("glibc 2.34", "musl 1.2"):
                with patch.object(build_install, "output", return_value=libc):
                    with self.assertRaisesRegex(ValueError, "glibc 2.35"):
                        build_install.host_target()

    def test_mac_install_verifies_stapled_copy_before_replacing_existing_app(self):
        app_dir = self.root / "Applications"
        old = app_dir / "Muxy Beta.app"
        old.mkdir(parents=True)
        (old / "old-marker").write_text("keep")

        def fail_validation(*args, **kwargs):
            if args[0] == "ditto":
                import shutil
                shutil.copytree(args[1], args[2])
            else:
                raise subprocess.CalledProcessError(1, args)

        with patch.object(build_install, "run", side_effect=fail_validation):
            with self.assertRaises(subprocess.CalledProcessError):
                build_install.install(self.artifact, self.bin_dir, app_dir)
        self.assertEqual((old / "old-marker").read_text(), "keep")

    def test_release_env_does_not_execute_or_expand_values(self):
        marker = self.root / "must-not-exist"
        value = f"$(touch '{marker}') `${{HOME}}`"
        env_file = self.root / ".env.release"
        env_file.write_text(f"DUMMY={value}\n")
        with patch.dict(os.environ, {}, clear=True):
            build_install.load_release_env(env_file)
            self.assertEqual(os.environ["DUMMY"], value)
        self.assertFalse(marker.exists())

    def test_release_env_errors_do_not_expose_values_or_partially_load(self):
        env_file = self.root / ".env.release"
        for invalid in ("not-an-assignment-secret", "DUMMY='unterminated-secret", "DUMMY=secret\0"):
            with self.subTest(invalid=invalid), patch.dict(os.environ, {}, clear=True):
                env_file.write_text("FIRST=dummy\n" + invalid + "\n")
                with self.assertRaises(ValueError) as error:
                    build_install.load_release_env(env_file)
                self.assertNotIn("secret", str(error.exception))
                self.assertNotIn("FIRST", os.environ)


if __name__ == "__main__":
    unittest.main()
