import importlib.util
import os
import plistlib
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

    def test_refuses_command_directory_before_replacing_installation(self):
        build_install.install(self.artifact, self.bin_dir)
        (self.bin_dir / "muxy-server").unlink()
        (self.bin_dir / "muxy-server").mkdir()
        (self.artifact / "muxy").write_text("updated")
        with self.assertRaisesRegex(ValueError, "Cannot replace"):
            build_install.install(self.artifact, self.bin_dir)
        self.assertIn("new-muxy", (self.bin_dir / "muxy").read_text())

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

    def test_identity_selection_requires_developer_id_and_is_unambiguous(self):
        first, second = "A" * 40, "B" * 40
        with patch.object(build_install, "output", return_value=(
            f'1) {first} "Developer ID Application: One (TEAM)"\n'
            f'2) {second} "Developer ID Application: Two (TEAM)"\n'
        )):
            self.assertEqual(build_install.signing_identity(first), first)
            self.assertEqual(build_install.signing_identity("Developer ID Application: Two (TEAM)"), second)
            for identity in (None, "-", "unknown"):
                with self.assertRaises(ValueError):
                    build_install.signing_identity(identity)

    def test_mac_bundle_uses_checkout_version_and_signs_all_executables(self):
        (self.artifact / "muxy-app").write_text("desktop")
        with patch.object(build_install, "run") as run:
            app = build_install.package_app(self.root, self.artifact, "2.0.0-beta-0", "identity")
        info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
        self.assertEqual(info["MuxyVersion"], "2.0.0-beta-0")
        self.assertEqual(info["CFBundleVersion"], "0")
        self.assertEqual(info["CFBundleIdentifier"], "com.muxy-beta.app")
        signed = [call.args for call in run.call_args_list if call.args[:2] == ("codesign", "--force")]
        self.assertEqual([args[-1].name for args in signed], ["muxy", "muxy-server", "muxy-app", "Muxy Beta.app"])
        self.assertTrue(all("runtime" in args and "--timestamp" in args for args in signed))

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

    def test_mac_missing_credentials_fails_before_build(self):
        with patch.dict(os.environ, {}, clear=True), \
                patch.object(build_install, "ROOT", self.root), \
                patch.object(sys, "argv", ["build-install.py"]), \
                patch.object(build_install, "host_target", return_value=("Darwin", "aarch64-apple-darwin")), \
                patch.object(build_install.shutil, "which", return_value="tool"), \
                patch.object(build_install, "signing_identity", return_value="identity"), \
                patch.object(build_install, "run") as run:
            with self.assertRaisesRegex(ValueError, "APPLE_ID"):
                build_install.main()
            run.assert_not_called()

    def test_linux_builds_only_cli_and_server_without_notarization(self):
        fake_root = self.root / "checkout"
        binaries = fake_root / "target/aarch64-unknown-linux-gnu/release"
        binaries.mkdir(parents=True)
        for name in build_install.BINARIES:
            (binaries / name).write_text("binary")
        with patch.object(build_install, "ROOT", fake_root), \
                patch.object(sys, "argv", ["build-install.py", "--install-dir", str(self.bin_dir)]), \
                patch.object(build_install, "host_target", return_value=("Linux", "aarch64-unknown-linux-gnu")), \
                patch.object(build_install.shutil, "which", return_value="tool"), \
                patch.object(build_install, "output", return_value='{"version":"2.0.0-beta-0","compatibility":15}'), \
                patch.object(build_install, "run") as run, \
                patch.object(build_install, "install") as install:
            build_install.main()
        self.assertEqual(run.call_count, 1)
        self.assertEqual(run.call_args.args, (
            "cargo", "build", "--locked", "--release", "--target", "aarch64-unknown-linux-gnu",
            "-p", "muxy-cli", "-p", "muxy-server",
        ))
        self.assertIsNone(install.call_args.args[2])

    def test_notarization_failure_prevents_install(self):
        fake_root = self.root / "checkout"

        def fail_notarization(*args, **kwargs):
            if args[0] == "bash":
                raise subprocess.CalledProcessError(1, args)

        with patch.object(build_install, "ROOT", fake_root), \
                patch.object(sys, "argv", ["build-install.py", "--keychain-profile", "local-notary"]), \
                patch.object(build_install, "host_target", return_value=("Darwin", "aarch64-apple-darwin")), \
                patch.object(build_install.shutil, "which", return_value="tool"), \
                patch.object(build_install, "signing_identity", return_value="identity"), \
                patch.object(build_install, "output", return_value='{"version":"2.0.0-beta-0","compatibility":15}'), \
                patch.object(build_install, "run", side_effect=fail_notarization) as run, \
                patch.object(build_install, "package_app", return_value=self.artifact), \
                patch.object(build_install, "install") as install:
            with self.assertRaises(subprocess.CalledProcessError):
                build_install.main()
            install.assert_not_called()
        self.assertEqual(run.call_args.kwargs["env"]["NOTARY_KEYCHAIN_PROFILE"], "local-notary")

    def test_release_env_is_optional_and_preserves_existing_shell_values(self):
        env_file = self.root / ".env.release"
        with patch.dict(os.environ, {"APPLE_ID": "shell@example.invalid"}, clear=True):
            build_install.load_release_env(env_file)
            self.assertEqual(dict(os.environ), {"APPLE_ID": "shell@example.invalid"})
            env_file.write_text(
                "# Dummy release configuration\n\n"
                "APPLE_ID=file@example.invalid\n"
                "export APPLE_TEAM_ID = 'TEST TEAM' # comment\n"
                'SIGN_IDENTITY="Developer ID Application: Test (TEAM)"\n'
                "APPLE_APP_SPECIFIC_PASSWORD='dummy$literal#value'\n"
                "HASH=abc#def # trailing comment\n"
                "EMPTY=\nQUOTED_EMPTY=\"\"\n"
            )
            build_install.load_release_env(env_file)
            self.assertEqual(dict(os.environ), {
                "APPLE_ID": "shell@example.invalid", "APPLE_TEAM_ID": "TEST TEAM",
                "SIGN_IDENTITY": "Developer ID Application: Test (TEAM)",
                "APPLE_APP_SPECIFIC_PASSWORD": "dummy$literal#value", "HASH": "abc#def",
                "EMPTY": "", "QUOTED_EMPTY": "",
            })

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
                self.assertIn(".env.release at line 2", str(error.exception))
                self.assertNotIn("secret", str(error.exception))
                self.assertNotIn("FIRST", os.environ)

    def test_main_loads_root_release_env_before_selecting_signing_credentials(self):
        (self.root / ".env.release").write_text(
            "SIGN_IDENTITY=file-identity\nNOTARY_KEYCHAIN_PROFILE=file-profile\n"
        )
        for options, identity, profile in (
            ([], "file-identity", "file-profile"),
            (["--sign-identity", "cli-identity", "--keychain-profile", "cli-profile"], "cli-identity", "cli-profile"),
        ):
            with self.subTest(options=options), patch.dict(os.environ, {}, clear=True), \
                    patch.object(build_install, "ROOT", self.root), \
                    patch.object(sys, "argv", ["build-install.py", *options]), \
                    patch.object(build_install, "host_target", return_value=("Darwin", "aarch64-apple-darwin")), \
                    patch.object(build_install.shutil, "which", return_value="tool"), \
                    patch.object(build_install, "signing_identity", return_value="identity") as signing, \
                    patch.object(build_install, "output", return_value='{"version":"2.0.0-beta-0","compatibility":15}'), \
                    patch.object(build_install, "run") as run, \
                    patch.object(build_install, "package_app", return_value=self.artifact), \
                    patch.object(build_install, "install"):
                build_install.main()
                signing.assert_called_once_with(identity)
                self.assertEqual(run.call_args.kwargs["env"]["NOTARY_KEYCHAIN_PROFILE"], profile)


if __name__ == "__main__":
    unittest.main()
