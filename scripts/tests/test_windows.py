import contextlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("windows", ROOT / "scripts/check-windows.py")
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)

TARGET = "aarch64-pc-windows-msvc"
WORKFLOW = (ROOT / ".github/workflows/ci.yml").read_text()


class WindowsTests(unittest.TestCase):
    def environment(self, system="Windows", host=TARGET, zig="C:\\zig\\zig.exe", env=None, part=None,
                    scope=None):
        stack = contextlib.ExitStack()
        self.addCleanup(stack.close)
        arguments = ["--part", part] if part else []
        arguments += ["--scope", scope] if scope else []
        stack.enter_context(patch.object(sys, "argv", ["check-windows.py", "--target", TARGET, *arguments]))
        stack.enter_context(patch.object(CHECK.platform, "system", return_value=system))
        stack.enter_context(patch.object(CHECK.shutil, "which", return_value=zig))
        stack.enter_context(patch.dict(os.environ, env or {}, clear=True))
        stack.enter_context(contextlib.redirect_stdout(io.StringIO()))

        stack.enter_context(patch.object(CHECK.subprocess, "check_output", return_value=f"host: {host}\n"))
        return stack.enter_context(patch.object(CHECK.slow_test_guard, "run"))

    def test_only_the_requested_native_windows_host_with_zig_can_run_the_check(self):
        for settings in [
            {"system": "Linux"},
            {"host": "x86_64-pc-windows-msvc"},
            {"host": "aarch64-pc-windows-gnullvm"},
            {"zig": None},
        ]:
            with self.subTest(settings=settings):
                run = self.environment(**settings)
                with self.assertRaises(ValueError):
                    CHECK.main()
                run.assert_not_called()
                self.doCleanups()

    def test_any_failed_check_stops_the_run(self):
        for failed_step in range(3):
            with self.subTest(failed_step=failed_step):
                run = self.environment()
                run.side_effect = [None] * failed_step + [subprocess.CalledProcessError(1, "check")]
                with self.assertRaises(subprocess.CalledProcessError):
                    CHECK.main()
                self.assertEqual(run.call_count, failed_step + 1)
                self.doCleanups()

    def test_check_covers_the_whole_workspace(self):
        run = self.environment()
        CHECK.main()
        self.assertEqual([call.args[0] for call in run.call_args_list], [*CHECK.PARTS["lint"], *CHECK.PARTS["test"]])
        for call in run.call_args_list:
            self.assertEqual(call.kwargs["cwd"], ROOT)

    def test_each_part_runs_alone(self):
        run = self.environment(part="lint")
        CHECK.main()
        self.assertEqual([call.args[0] for call in run.call_args_list], CHECK.PARTS["lint"])
        self.doCleanups()
        run = self.environment(part="test", scope="desktop")
        CHECK.main()
        self.assertEqual([call.args[0] for call in run.call_args_list], [CHECK.ci_scope.DESKTOP_TEST])


    def test_ci_runs_the_windows_check_on_x86_64(self):
        for expected in [
            "python scripts/check-windows.py --target x86_64-pc-windows-msvc\n"
            '          --part "${{ matrix.part }}" --scope "$env:SCOPE"',
            "runs-on: windows-latest\n",
            "git config --global core.autocrlf false\n          git config --global core.longpaths true\n"
            "      - uses: actions/checkout@v4\n",
        ]:
            self.assertIn(expected, WORKFLOW)



PACKAGE_SPEC = importlib.util.spec_from_file_location("package_windows", ROOT / "scripts/package-windows.py")
PACKAGE = importlib.util.module_from_spec(PACKAGE_SPEC)
PACKAGE_SPEC.loader.exec_module(PACKAGE)


class WindowsPackageTests(unittest.TestCase):
    def test_a_release_zips_the_three_checked_programs_with_the_license(self):
        with tempfile.TemporaryDirectory() as directory:
            binaries = Path(directory) / "release"
            binaries.mkdir()
            for name in PACKAGE.BINARIES:
                (binaries / name).write_bytes(name.encode())
            with patch.object(PACKAGE.release, "check_build") as check:
                archive = PACKAGE.package("2.1.0-beta.3", "arm64", binaries, Path(directory) / "out")
            self.assertEqual(archive.name, "muxy-2.1.0-beta.3-windows-arm64.zip")
            self.assertEqual(
                [call.args for call in check.call_args_list],
                [("2.1.0-beta.3", binaries / name) for name in PACKAGE.BINARIES],
            )
            with zipfile.ZipFile(archive) as zipped:
                self.assertEqual(sorted(zipped.namelist()), sorted(
                    f"muxy-2.1.0-beta.3-windows-arm64/{name}" for name in (*PACKAGE.BINARIES, "LICENSE")))

    def test_a_build_that_reports_another_release_is_not_packaged(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(PACKAGE.release, "check_build", side_effect=ValueError("mismatch")):
                with self.assertRaises(ValueError):
                    PACKAGE.package("2.1.0-beta.3", "x86_64", Path(directory), Path(directory) / "out")
            self.assertFalse((Path(directory) / "out").exists())

    def test_only_betas_publish_windows_archives(self):
        publish = (ROOT / "scripts/publish-release.sh").read_text()
        self.assertIn('if [[ "$CHANNEL" == beta ]]; then\n    for ARCH in arm64 x86_64; do\n'
                      '        ASSETS+=("muxy-${VERSION}-windows-${ARCH}.zip")', publish)
        self.assertIn("windows: true", (ROOT / ".github/workflows/release-beta.yml").read_text())
        self.assertNotIn("windows:", (ROOT / ".github/workflows/release.yml").read_text())
        self.assertIn("if: inputs.windows", (ROOT / ".github/workflows/build-release.yml").read_text())


if __name__ == "__main__":
    unittest.main()
