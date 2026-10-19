import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("windows", ROOT / "scripts/check-windows.py")
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)

TARGET = "aarch64-pc-windows-msvc"
BUILD = {"version": "2.0.0", "compatibility": 1, "protocol": [1]}
WORKFLOW = (ROOT / ".github/workflows/ci.yml").read_text()


class WindowsTests(unittest.TestCase):
    def environment(self, system="Windows", host=TARGET, zig="C:\\zig\\zig.exe", env=None, builds=None):
        builds = builds or {}
        stack = contextlib.ExitStack()
        self.addCleanup(stack.close)
        stack.enter_context(patch.object(sys, "argv", ["check-windows.py", "--target", TARGET]))
        stack.enter_context(patch.object(CHECK.platform, "system", return_value=system))
        stack.enter_context(patch.object(CHECK.shutil, "which", return_value=zig))
        stack.enter_context(patch.dict(os.environ, {"RUSTDOCFLAGS": "--cfg regression_guard", **(env or {})},
                                       clear=True))
        stack.enter_context(contextlib.redirect_stdout(io.StringIO()))

        def output(command, **kwargs):
            if command[0] == "rustc":
                return f"host: {host}\n"
            return json.dumps(builds.get(Path(command[0]).stem, BUILD)).encode()

        reports = stack.enter_context(patch.object(CHECK.subprocess, "check_output", side_effect=output))
        return stack.enter_context(patch.object(CHECK.subprocess, "run")), reports

    def test_only_the_requested_native_windows_host_with_zig_can_run_the_check(self):
        for settings in [
            {"system": "Linux"},
            {"host": "x86_64-pc-windows-msvc"},
            {"host": "aarch64-pc-windows-gnullvm"},
            {"env": {"CARGO_BUILD_TARGET": "x86_64-pc-windows-msvc"}},
            {"env": {"CARGO_BUILD_TARGET": TARGET}},
            {"zig": None},
        ]:
            with self.subTest(settings=settings):
                run, _ = self.environment(**settings)
                with self.assertRaises(ValueError):
                    CHECK.main()
                run.assert_not_called()
                self.doCleanups()

    def test_any_failed_check_stops_the_run(self):
        for failed_step in range(6):
            with self.subTest(failed_step=failed_step):
                run, reports = self.environment()
                run.side_effect = [None] * failed_step + [subprocess.CalledProcessError(1, "check")]
                with self.assertRaises(subprocess.CalledProcessError):
                    CHECK.main()
                self.assertEqual(run.call_count, failed_step + 1)
                self.assertTrue(all(call.kwargs["check"] for call in run.call_args_list))
                self.assertEqual(reports.call_count, 1)
                self.doCleanups()

    def test_check_covers_the_whole_workspace_and_the_built_binaries(self):
        run, reports = self.environment(env={"CARGO_TARGET_DIR": "C:\\build"})
        CHECK.main()
        self.assertEqual([call.args[0] for call in run.call_args_list], [
            [sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests"],
            ["cargo", "fmt", "--all", "--check"],
            ["cargo", "clippy", "--locked", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"],
            ["cargo", "test", "--locked", "--workspace", "--all-features"],
            ["cargo", "doc", "--locked", "--workspace", "--no-deps"],
            ["cargo", "build", "--locked", "-p", "muxy-app", "-p", "muxy-cli", "-p", "muxy-server"],
        ])
        for call in run.call_args_list:
            self.assertEqual(call.kwargs["cwd"], ROOT)
            self.assertEqual(call.kwargs["env"]["RUSTDOCFLAGS"], "--cfg regression_guard -D warnings")
        binaries = ROOT / "C:\\build" / "debug"
        self.assertEqual([call.args[0] for call in reports.call_args_list[1:]], [
            [binaries / "muxy-app.exe", "--build-info"],
            [binaries / "muxy.exe", "--build-info"],
            [binaries / "muxy-server.exe", "--build-info"],
        ])

    def test_binaries_are_read_from_the_configured_target_directory(self):
        for env, directory in [({}, "target"), ({"CARGO_BUILD_TARGET_DIR": "C:\\other"}, "C:\\other")]:
            with self.subTest(env=env):
                _, reports = self.environment(env=env)
                CHECK.main()
                self.assertEqual(reports.call_args_list[1].args[0][0], ROOT / directory / "debug" / "muxy-app.exe")
                self.doCleanups()

    def test_binaries_that_report_different_builds_fail_the_check(self):
        for name in CHECK.BINARIES:
            with self.subTest(name=name):
                self.environment(builds={name: {**BUILD, "version": "1.0.0"}})
                with self.assertRaisesRegex(ValueError, "different builds"):
                    CHECK.main()
                self.doCleanups()

    def test_ci_runs_the_windows_check_on_both_architectures(self):
        for expected in [
            'python scripts/check-windows.py --target "${{ matrix.target }}"',
            "runner: windows-latest\n            target: x86_64-pc-windows-msvc\n",
            "runner: windows-11-arm\n            target: aarch64-pc-windows-msvc\n",
            "git config --global core.autocrlf false\n          git config --global core.longpaths true\n"
            "      - uses: actions/checkout@v4\n",
            "'windows/**'",
        ]:
            self.assertIn(expected, WORKFLOW)


if __name__ == "__main__":
    unittest.main()
