import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("ci_scope", ROOT / "scripts/ci_scope.py")
SCOPE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCOPE)

WORKFLOW = (ROOT / ".github/workflows/ci.yml").read_text()
RELEASE_WORKFLOW = (ROOT / ".github/workflows/build-release.yml").read_text()
GIT_ENV = {
    **os.environ,
    "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@example.com",
    "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@example.com",
    "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_SYSTEM": os.devnull,
}


class Repository:
    def __init__(self, directory):
        self.path = Path(directory)
        self.git("init", "--quiet", "--initial-branch=main")

    def git(self, *args):
        subprocess.run(["git", *args], cwd=self.path, env=GIT_ENV, check=True, capture_output=True)

    def write(self, name, text="content\n"):
        file = self.path / name
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(text)

    def commit(self, message="change"):
        self.git("add", "--all")
        self.git("commit", "--quiet", "-m", message)

    def test_merge(self):
        self.commit("pull request")
        self.git("checkout", "--quiet", "main")
        self.git("merge", "--quiet", "--no-ff", "-m", "test merge", "pull")


class ClassifyTests(unittest.TestCase):
    def test_only_desktop_crate_sources_narrow_the_scope(self):
        for paths in [
            ["crates/muxy-app/src/main.rs"],
            ["crates/muxy-ui/src/lib.rs"],
            ["crates/muxy-app/tests/flow.rs", "crates/muxy-ui/assets/icon.svg"],
        ]:
            with self.subTest(paths=paths):
                self.assertEqual(SCOPE.classify(paths), SCOPE.DESKTOP)

    def test_anything_else_runs_everything(self):
        for paths in [
            [],
            ["Cargo.lock"],
            ["Cargo.toml"],
            ["rust-toolchain.toml"],
            [".github/workflows/ci.yml"],
            ["scripts/ci_scope.py"],
            ["vendor/libghostty-vt-sys/build.rs"],
            ["docs/tech/README.md"],
            ["crates/muxy-app/Cargo.toml"],
            ["crates/muxy-ui/Cargo.toml"],
            ["crates/muxy-app-core/src/lib.rs"],
            ["crates/muxy-core/src/lib.rs"],
            ["crates/muxy-app"],
            ["crates/muxy-app/src/main.rs", "crates/muxy-server/src/main.rs"],
            ["crates/muxy-app/src/main.rs", "Cargo.lock"],
        ]:
            with self.subTest(paths=paths):
                self.assertEqual(SCOPE.classify(paths), SCOPE.FULL)

    def test_scope_picks_the_test_commands(self):
        full = [["cargo", "test", "--workspace"]]
        self.assertEqual(SCOPE.tests(SCOPE.FULL, full), full)
        self.assertEqual(SCOPE.tests(SCOPE.DESKTOP, full), [SCOPE.DESKTOP_TEST])
        self.assertEqual(SCOPE.DESKTOP_TEST[:3], ["cargo", "test", "--locked"])
        for crate in SCOPE.DESKTOP_CRATES:
            self.assertIn(crate, SCOPE.DESKTOP_TEST)

    def test_no_other_crate_depends_on_the_desktop_crates(self):
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--locked", "--no-deps", "--all-features", "--format-version=1"], cwd=ROOT))
        names = {package["name"] for package in metadata["packages"]}
        self.assertLessEqual(set(SCOPE.DESKTOP_CRATES), names)
        for package in metadata["packages"]:
            if package["name"] in SCOPE.DESKTOP_CRATES:
                continue
            dependencies = {dependency["name"] for dependency in package["dependencies"]}
            with self.subTest(package=package["name"]):
                self.assertFalse(dependencies & set(SCOPE.DESKTOP_CRATES))


class ChangedPathsTests(unittest.TestCase):
    def repository(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        repository = Repository(temporary.name)
        repository.write("crates/muxy-core/src/moved.rs")
        repository.write("crates/muxy-app/src/main.rs")
        repository.commit("base")
        repository.git("checkout", "--quiet", "-b", "pull")
        return repository

    def scope(self, repository):
        return SCOPE.classify(SCOPE.changed_paths(repository.path))

    def test_a_desktop_only_pull_request_narrows_the_scope(self):
        repository = self.repository()
        repository.write("crates/muxy-app/src/main.rs", "changed\n")
        repository.write("crates/muxy-ui/src/lib.rs")
        repository.test_merge()
        self.assertEqual(self.scope(repository), SCOPE.DESKTOP)

    def test_changes_on_the_base_branch_do_not_widen_the_scope(self):
        repository = self.repository()
        repository.write("crates/muxy-app/src/main.rs", "changed\n")
        repository.commit("pull request")
        repository.git("checkout", "--quiet", "main")
        repository.write("crates/muxy-core/src/other.rs")
        repository.commit("base moved on")
        repository.git("merge", "--quiet", "--no-ff", "-m", "test merge", "pull")
        self.assertEqual(self.scope(repository), SCOPE.DESKTOP)

    def test_moving_a_file_out_of_a_shared_crate_runs_everything(self):
        repository = self.repository()
        repository.git("mv", "crates/muxy-core/src/moved.rs", "crates/muxy-app/src/moved.rs")
        repository.test_merge()
        self.assertIn("crates/muxy-core/src/moved.rs", SCOPE.changed_paths(repository.path))
        self.assertEqual(self.scope(repository), SCOPE.FULL)

    def test_deleting_a_shared_file_runs_everything(self):
        repository = self.repository()
        repository.git("rm", "--quiet", "crates/muxy-core/src/moved.rs")
        repository.write("crates/muxy-app/src/main.rs", "changed\n")
        repository.test_merge()
        self.assertEqual(self.scope(repository), SCOPE.FULL)

    def test_unusual_file_names_are_read_exactly(self):
        repository = self.repository()
        repository.write("crates/muxy-app/src/naïve file.rs")
        repository.test_merge()
        self.assertEqual(SCOPE.changed_paths(repository.path), ["crates/muxy-app/src/naïve file.rs"])

    def test_anything_but_a_test_merge_runs_everything(self):
        repository = self.repository()
        repository.write("crates/muxy-app/src/main.rs", "changed\n")
        repository.commit("not a merge")
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(self.scope(repository), SCOPE.FULL)

    def test_an_empty_merge_runs_everything(self):
        repository = self.repository()
        repository.git("commit", "--quiet", "--allow-empty", "-m", "nothing")
        repository.git("checkout", "--quiet", "main")
        repository.git("merge", "--quiet", "--no-ff", "-m", "test merge", "pull")
        self.assertEqual(self.scope(repository), SCOPE.FULL)

    def test_a_shallow_checkout_too_short_to_see_the_parents_runs_everything(self):
        repository = self.repository()
        repository.write("crates/muxy-app/src/main.rs", "changed\n")
        repository.test_merge()
        with tempfile.TemporaryDirectory() as directory, contextlib.redirect_stderr(io.StringIO()):
            subprocess.run(
                ["git", "clone", "--quiet", "--depth=1", repository.path.as_uri(), directory],
                env=GIT_ENV, check=True, capture_output=True)
            self.assertEqual(SCOPE.classify(SCOPE.changed_paths(Path(directory))), SCOPE.FULL)

    def test_a_ci_checkout_of_depth_two_sees_the_change(self):
        repository = self.repository()
        repository.write("crates/muxy-app/src/main.rs", "changed\n")
        repository.test_merge()
        with tempfile.TemporaryDirectory() as directory:
            subprocess.run(
                ["git", "clone", "--quiet", "--depth=2", repository.path.as_uri(), directory],
                env=GIT_ENV, check=True, capture_output=True)
            self.assertEqual(SCOPE.classify(SCOPE.changed_paths(Path(directory))), SCOPE.DESKTOP)

    def test_a_failing_git_runs_everything(self):
        with tempfile.TemporaryDirectory() as directory, contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(SCOPE.classify(SCOPE.changed_paths(Path(directory))), SCOPE.FULL)


class WorkflowTests(unittest.TestCase):
    def job(self, name):
        match = re.search(rf"^  {re.escape(name)}:\n((?:(?:    .*)?\n)*)", WORKFLOW, re.MULTILINE)
        self.assertIsNotNone(match, name)
        return match.group(1)

    def test_only_pull_requests_are_scoped(self):
        self.assertIn("if: github.event_name == 'pull_request'", self.job("scope"))

    def test_a_failed_or_skipped_scope_job_still_runs_the_whole_workspace(self):
        for name in ["macos", "linux", "linux-desktop"]:
            job = self.job(name)
            with self.subTest(job=name):
                self.assertIn("needs: scope", job)
                self.assertRegex(job, r"if: \$\{\{ !cancelled\(\)")
        for name in ["macos", "linux-desktop"]:
            with self.subTest(job=name):
                self.assertIn("SCOPE: ${{ needs.scope.outputs.scope || 'full' }}", self.job(name))
                self.assertIn('--scope "$SCOPE"', self.job(name))

    def test_only_an_explicit_desktop_scope_skips_the_headless_job(self):
        self.assertIn("if: ${{ !cancelled() && needs.scope.outputs.scope != 'desktop' }}", self.job("linux"))
        for name in ["macos", "linux-desktop"]:
            with self.subTest(job=name):
                self.assertIn("if: ${{ !cancelled() }}", self.job(name))

    def test_the_ghostty_build_is_cached_once_instead_of_every_run(self):
        self.assertIn("LIBGHOSTTY_VT_SYS_ZIG_CACHE_DIR: ${{ github.workspace }}/.zig-cache", WORKFLOW)
        for name in ["macos", "linux", "linux-desktop"]:
            job = self.job(name)
            with self.subTest(job=name):
                self.assertIn("use-cache: false", job)
                self.assertIn("path: .zig-cache", job)
                self.assertIn("key: zig-${{ matrix.runner }}-0.17.0-${{ hashFiles('vendor/libghostty-vt-sys/**') }}", job)

    def test_fork_pull_requests_stay_on_github_hosted_macos_runners(self):
        job = self.job("macos")
        self.assertIn(
            "${{ github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name != github.repository"
            " && 'macos-15' || vars.MACOS_CI_RUNNER || 'macos-15' }}",
            job,
        )
        self.assertIn("runs-on: ${{ matrix.runner }}", job)

    def test_macos_caches_are_kept_per_runner(self):
        self.assertIn("key: ${{ matrix.part }}-${{ matrix.runner }}", self.job("macos"))

    def test_release_builds_neither_read_nor_spend_the_shared_build_caches(self):
        self.assertNotIn("rust-cache", RELEASE_WORKFLOW)
        self.assertNotIn("actions/cache", RELEASE_WORKFLOW)
        steps = RELEASE_WORKFLOW.count("uses: mlugg/setup-zig@v2")
        self.assertGreater(steps, 0)
        self.assertEqual(RELEASE_WORKFLOW.count("use-cache: false"), steps)
        self.assertNotIn("MACOS_CI_RUNNER", RELEASE_WORKFLOW)


if __name__ == "__main__":
    unittest.main()
