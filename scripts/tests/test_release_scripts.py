import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SHA = "a" * 40
VERSION = "2.0.0-beta-1234"
MOBILE_SDK = [f"muxy-mobile-{VERSION}-ios.zip", f"muxy-mobile-{VERSION}-android.zip", f"muxy-mobile-{VERSION}.json"]

FAKE_TOOL = r'''
import json, os, sys
from pathlib import Path
name = Path(sys.argv[0]).name
args = sys.argv[1:]
with open(os.environ["TOOL_LOG"], "a") as log:
    log.write(json.dumps([name, *args]) + "\n")
if name == "git":
    if args[0] == "-C":
        args = args[2:]
    if args == ["rev-parse", "HEAD"]:
        print(os.environ["GITHUB_SHA"])
    elif args == ["rev-parse", "--is-shallow-repository"]:
        print("false")
    elif args == ["rev-list", "--count", "HEAD"]:
        print("1234")
    elif args[0] == "fetch":
        sys.exit(int(os.environ.get("FETCH_EXIT", "0")))
    elif args[0] == "merge-base":
        sys.exit(int(os.environ.get("ANCESTOR_EXIT", "0")))
    elif args[0] == "show-ref":
        sys.exit(0 if os.environ.get("TAG_SHA") else 1)
    elif args[0] == "rev-parse":
        print(os.environ["TAG_SHA"])
    elif args[0] == "describe":
        previous = os.environ.get("PREVIOUS_TAG")
        if not previous:
            sys.exit(1)
        print(previous)
    else:
        sys.exit("unexpected git arguments: " + repr(args))
elif name == "gh":
    if args[:2] == ["release", "view"] and args[-1] == "assets":
        assets = [{"name": p.name, "size": p.stat().st_size} for p in Path.cwd().iterdir()
                  if p.is_file() and p.name != os.environ.get("MISSING_REMOTE_ASSET")]
        print(json.dumps({"assets": assets})); sys.exit(0)
    if args[:2] == ["release", "view"]:
        channel = args[2] == "beta-2.x"
        state = os.environ.get("CHANNEL_STATE" if channel else "RELEASE_STATE", "missing")
        if state == "missing":
            sys.exit(1)
        print(json.dumps({"isDraft": state == "draft", "isPrerelease": state != "stable",
                          "assets": [{"name": "update.json"}] if os.environ.get("CHANNEL_VERSION") else [],
                          "targetCommitish": os.environ["GITHUB_SHA"]}))
    elif args[:2] == ["release", "download"]:
        if os.environ.get("DOWNLOAD_EXIT"):
            sys.exit(1)
        version = os.environ["CHANNEL_VERSION"] if args[2] == "beta-2.x" else args[2][1:]
        directory = Path(args[args.index("--dir") + 1])
        metadata = {"schema": 1, "version": version, "platforms": {
            "macos-" + platform: {"url": f"https://github.com/example/muxy/releases/download/v{version}/Muxy-{version}-{arch}.dmg", "size": len(arch)}
            for platform, arch in [("aarch64", "arm64"), ("x86_64", "x86_64")]
        }}
        (directory / "update.json").write_text(json.dumps(metadata))
    elif args[:2] == ["release", "upload"]:
        sys.exit(int(os.environ.get("UPLOAD_EXIT", "0")))
    elif args[0] == "api":
        previous = next(arg.split("=", 1)[1] for arg in args if arg.startswith("previous_tag_name="))
        print("Generated changes since " + previous)
    elif args[:2] not in (["release", "create"], ["release", "edit"]):
        sys.exit("unexpected gh arguments: " + repr(args))
elif name == "xcrun":
    if args[:2] == ["notarytool", "submit"]:
        submits = sum(json.loads(line)[:3] == ["xcrun", "notarytool", "submit"]
                      for line in Path(os.environ["TOOL_LOG"]).read_text().splitlines())
        if submits <= int(os.environ.get("NOTARY_NETWORK_FAILURES", "0")):
            print("NSURLErrorDomain Code=-1009: no network route", file=sys.stderr)
            sys.exit(7)
        if os.environ.get("NOTARY_AUTH_FAILURE"):
            print("Invalid credentials", file=sys.stderr)
            sys.exit(9)
        print(json.dumps({"id": "test-submission", "status": os.environ.get("NOTARY_STATUS", "Accepted")}))
        sys.exit(int(os.environ.get("NOTARY_EXIT", "0")))
    elif args[:2] == ["notarytool", "log"]:
        print("{}")
    elif args[0] != "stapler":
        sys.exit("unexpected xcrun arguments: " + repr(args))
elif name == "codesign":
    sys.exit(int(os.environ.get("CODESIGN_EXIT", "0")))
elif name == "spctl":
    sys.exit(int(os.environ.get("SPCTL_EXIT", "0")))
elif name == "sleep":
    pass
elif name == "ditto":
    Path(args[-1]).write_bytes(b"app archive")
else:
    sys.exit("unexpected tool: " + name)
'''


class ReleaseScriptTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.tools = self.directory / "tools"
        self.tools.mkdir()
        for name in ("git", "gh", "xcrun", "spctl", "codesign", "sleep", "ditto"):
            tool = self.tools / name
            tool.write_text(f"#!{sys.executable}\n" + FAKE_TOOL)
            tool.chmod(0o755)
        self.log = self.directory / "tools.jsonl"
        self.env = {
            **os.environ,
            "PATH": str(self.tools) + os.pathsep + os.environ["PATH"],
            "TOOL_LOG": str(self.log),
            "GITHUB_REPOSITORY": "example/muxy",
            "GITHUB_SHA": SHA,
            "GITHUB_REF": "refs/heads/2.x",
            "APPLE_ID": "test@example.invalid",
            "APPLE_APP_SPECIFIC_PASSWORD": "test-password",
            "APPLE_TEAM_ID": "test-team",
            "NOTARY_KEYCHAIN_PROFILE": "",
        }
        for arch in ("arm64", "x86_64"):
            (self.directory / f"Muxy-{VERSION}-{arch}.dmg").write_bytes(arch.encode())
            for system, extension in (("macos", "zip"), ("linux", "tar.gz")):
                (self.directory / f"muxy-{VERSION}-{system}-{arch}.{extension}").write_bytes(b"archive")
        for name in MOBILE_SDK:
            (self.directory / name).write_bytes(name.encode())
        (self.directory / "install-muxy.sh").write_bytes((ROOT / "scripts/install-muxy.sh").read_bytes())

    def run_script(self, script, *args):
        return subprocess.run(
            ["bash", str(ROOT / "scripts" / script), *map(str, args)],
            env=self.env, capture_output=True, text=True,
        )

    def calls(self, tool):
        if not self.log.exists():
            return []
        return [entry for line in self.log.read_text().splitlines()
                if (entry := json.loads(line))[0] == tool]

    def version_release_calls(self):
        return [call for call in self.calls("gh")
                if call[1] == "release" and call[2] != "download" and call[3] == f"v{VERSION}"]

    def publish(self):
        return self.run_script("publish-beta.sh", VERSION, self.directory)

    def test_publishes_both_architectures_at_exact_sha_without_latest(self):
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.version_release_calls()
        self.assertEqual([call[2] for call in calls], ["view", "create", "upload", "view", "edit"])
        for call in (calls[1], calls[4]):
            self.assertEqual(call[3], f"v{VERSION}")
            self.assertEqual(call[call.index("--target") + 1], SHA)
            self.assertIn("--prerelease", call)
            self.assertIn("--latest=false", call)
        for arch in ("arm64", "x86_64"):
            filename = f"Muxy-{VERSION}-{arch}.dmg"
            self.assertIn(filename, calls[2])
            self.assertIn(hashlib.sha256(arch.encode()).hexdigest(),
                          (self.directory / "SHA256SUMS").read_text())
        self.assertIn("--draft", calls[1])
        self.assertIn("--draft=false", calls[4])
        notes = (self.directory / "release-notes.md").read_text()
        self.assertIn("Rust/GPUI beta", notes)
        self.assertIn("Muxy Beta.app", notes)
        self.assertIn("Library/Application Support/Muxy Beta", notes)
        self.assertNotIn("alpha", notes.lower())

    def test_alpha_versions_are_rejected_before_publishing(self):
        result = self.run_script("publish-beta.sh", "2.0.0-alpha-1234", self.directory)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("release version must be 2.0.0-beta-", result.stderr)
        self.assertEqual(self.calls("gh"), [])

    def test_generated_notes_continue_from_previous_beta_or_alpha(self):
        for previous in ("v2.0.0-beta-1233", "v2.0.0-alpha-1233"):
            with self.subTest(previous=previous):
                self.log.unlink(missing_ok=True)
                self.env["PREVIOUS_TAG"] = previous
                result = self.publish()
                self.assertEqual(result.returncode, 0, result.stderr)
                describe = next(call for call in self.calls("git") if "describe" in call)
                self.assertIn("v2.0.0-beta-*", describe)
                self.assertIn("v2.0.0-alpha-*", describe)
                api = next(call for call in self.calls("gh") if call[1] == "api")
                self.assertIn("repos/example/muxy/releases/generate-notes", api)
                self.assertIn(f"tag_name=v{VERSION}", api)
                self.assertIn(f"previous_tag_name={previous}", api)
                self.assertIn("Generated changes since", (self.directory / "release-notes.md").read_text())

    def test_published_rerun_does_not_replace_assets(self):
        self.env.update(RELEASE_STATE="published", TAG_SHA=SHA)
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([call[2] for call in self.version_release_calls()], ["view"])

    def test_draft_rerun_resumes_upload_and_publish(self):
        self.env.update(RELEASE_STATE="draft")
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([call[2] for call in self.version_release_calls()], ["view", "upload", "view", "edit"])

    def test_feed_is_promoted_only_after_versioned_assets_are_published(self):
        self.assertEqual(self.publish().returncode, 0)
        calls = self.calls("gh")
        published = next(i for i, call in enumerate(calls) if call[2:4] == ["edit", f"v{VERSION}"])
        promoted = next(i for i, call in enumerate(calls) if call[2:4] == ["upload", "beta-2.x"])
        self.assertLess(published, promoted)
        metadata = json.loads((self.directory / "update.json").read_text())
        self.assertEqual(metadata["version"], VERSION)
        self.assertEqual(set(metadata["platforms"]), {"macos-aarch64", "macos-x86_64"})
        self.assertEqual(metadata["platforms"]["macos-aarch64"]["size"], 5)
        for call in calls:
            if call[2] in ("create", "edit"):
                self.assertIn("--prerelease", call)
                self.assertIn("--latest=false", call)

    def test_older_finishing_build_does_not_roll_back_the_feed(self):
        self.env.update(CHANNEL_STATE="published", CHANNEL_VERSION="2.0.0-beta-1235")
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any(call[2:4] == ["upload", "beta-2.x"] for call in self.calls("gh")))

    def test_newer_build_replaces_existing_feed(self):
        self.env.update(CHANNEL_STATE="published", CHANNEL_VERSION="2.0.0-beta-999")
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(any(call[2:4] == ["upload", "beta-2.x"] for call in self.calls("gh")))

    def test_published_rerun_resumes_feed_promotion_from_published_metadata(self):
        self.env.update(RELEASE_STATE="published", TAG_SHA=SHA)
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls("gh")
        self.assertTrue(any(call[2:4] == ["download", f"v{VERSION}"] for call in calls))
        self.assertTrue(any(call[2:4] == ["upload", "beta-2.x"] for call in calls))
        self.assertFalse(any(call[2:4] == ["upload", f"v{VERSION}"] for call in calls))

    def test_stable_channel_and_failed_metadata_download_are_never_promoted(self):
        for override in ({"CHANNEL_STATE": "stable"}, {"DOWNLOAD_EXIT": "1"}):
            with self.subTest(override=override):
                self.log.unlink(missing_ok=True)
                before = self.env.copy()
                self.env.update(override)
                self.assertNotEqual(self.publish().returncode, 0)
                self.assertFalse(any(call[2:4] == ["upload", "beta-2.x"] for call in self.calls("gh")))
                self.env = before

    def test_upload_failure_leaves_draft_unpublished(self):
        self.env["UPLOAD_EXIT"] = "1"
        self.assertNotEqual(self.publish().returncode, 0)
        self.assertEqual([call[2] for call in self.calls("gh")], ["view", "create", "upload"])

    def test_publishes_without_intel_macos_when_both_artifacts_are_absent(self):
        intel_assets = [f"Muxy-{VERSION}-x86_64.dmg", f"muxy-{VERSION}-macos-x86_64.zip"]
        for asset in intel_assets:
            (self.directory / asset).unlink()
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.version_release_calls()
        self.assertEqual([call[2] for call in calls], ["view", "create", "upload", "view", "edit"])
        for asset in intel_assets:
            self.assertNotIn(asset, calls[2])
        for arch in ("arm64", "x86_64"):
            self.assertIn(f"muxy-{VERSION}-linux-{arch}.tar.gz", calls[2])
        self.assertIn(f"Muxy-{VERSION}-arm64.dmg", calls[2])
        self.assertIn(f"muxy-{VERSION}-macos-arm64.zip", calls[2])
        metadata = json.loads((self.directory / "update.json").read_text())
        self.assertEqual(set(metadata["platforms"]), {"macos-aarch64"})
        self.assertEqual(len((self.directory / "SHA256SUMS").read_text().splitlines()), 9)
        self.assertNotIn("Intel", (self.directory / "release-notes.md").read_text())

    def test_incomplete_intel_artifacts_prevent_release(self):
        (self.directory / f"Muxy-{VERSION}-x86_64.dmg").unlink()
        self.assertNotEqual(self.publish().returncode, 0)
        self.assertEqual(self.calls("gh"), [])

    def test_missing_arm64_dmg_prevents_release(self):
        (self.directory / f"Muxy-{VERSION}-arm64.dmg").unlink()
        self.assertNotEqual(self.publish().returncode, 0)
        self.assertEqual(self.calls("gh"), [])

    def test_missing_any_standalone_target_or_installer_prevents_release(self):
        for asset in list(self.directory.glob("muxy-*")) + [self.directory / "install-muxy.sh"]:
            data = asset.read_bytes()
            asset.unlink()
            self.assertNotEqual(self.publish().returncode, 0)
            self.assertEqual(self.calls("gh"), [])
            asset.write_bytes(data)

    def test_missing_remote_asset_leaves_release_draft(self):
        self.env["MISSING_REMOTE_ASSET"] = f"muxy-{VERSION}-linux-arm64.tar.gz"
        result = self.publish()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing or incomplete uploaded asset", result.stderr)
        self.assertFalse(any(call[2] == "edit" for call in self.version_release_calls()))

    def test_hashes_cover_every_asset_and_notes_use_immutable_installer(self):
        self.assertEqual(self.publish().returncode, 0)
        checksums = (self.directory / "SHA256SUMS").read_text().splitlines()
        self.assertEqual(len(checksums), 11)
        for line in checksums:
            digest, name = line.split()
            self.assertEqual(digest, hashlib.sha256((self.directory / name).read_bytes()).hexdigest())
        notes = (self.directory / "release-notes.md").read_text()
        self.assertIn(f"curl -fsSL https://github.com/example/muxy/releases/download/v{VERSION}/install-muxy.sh | sh -s -- --version {VERSION}", notes)
        self.assertNotIn("releases/latest", notes)

    def test_mobile_sdk_is_published_and_hashed_next_to_the_apps(self):
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        upload = self.version_release_calls()[2]
        checksums = (self.directory / "SHA256SUMS").read_text()
        for name in MOBILE_SDK:
            self.assertIn(name, upload)
            self.assertIn(f"{hashlib.sha256(name.encode()).hexdigest()}  {name}", checksums)

    def test_wrong_branch_prevents_release(self):
        self.env["GITHUB_REF"] = "refs/heads/main"
        self.assertNotEqual(self.publish().returncode, 0)
        self.assertEqual(self.calls("gh"), [])

    def test_tag_collision_prevents_release(self):
        self.env["TAG_SHA"] = "b" * 40
        result = self.publish()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("different commit", result.stderr)
        self.assertEqual(self.calls("gh"), [])

    def test_rewritten_history_prevents_release(self):
        self.env["ANCESTOR_EXIT"] = "1"
        self.assertNotEqual(self.publish().returncode, 0)
        self.assertEqual(self.calls("gh"), [])

    def test_fetch_failure_is_not_treated_as_a_missing_tag(self):
        self.env["FETCH_EXIT"] = "1"
        self.assertNotEqual(self.publish().returncode, 0)
        self.assertEqual(self.calls("gh"), [])

    def test_stable_release_is_never_modified(self):
        self.env["RELEASE_STATE"] = "stable"
        self.assertNotEqual(self.publish().returncode, 0)
        self.assertEqual([call[2] for call in self.calls("gh")], ["view"])

    def notarize(self):
        return self.run_script("notarize-release.sh", self.directory / f"Muxy-{VERSION}-arm64.dmg")

    def test_accepted_notarization_is_stapled_and_assessed(self):
        result = self.notarize()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([call[1:3] for call in self.calls("xcrun")], [
            ["notarytool", "submit"], ["notarytool", "log"],
            ["stapler", "staple"], ["stapler", "validate"],
        ])
        self.assertEqual(len(self.calls("spctl")), 1)

    def test_app_is_archived_then_stapled_with_keychain_credentials(self):
        app = self.directory / "Muxy Beta.app"
        app.mkdir()
        self.env["NOTARY_KEYCHAIN_PROFILE"] = "local-notary"
        for key in ("APPLE_ID", "APPLE_APP_SPECIFIC_PASSWORD", "APPLE_TEAM_ID"):
            del self.env[key]
        result = self.run_script("notarize-release.sh", app)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls("ditto")[0][1:6], ["-c", "-k", "--sequesterRsrc", "--keepParent", str(app)])
        submit = self.calls("xcrun")[0]
        self.assertIn("--keychain-profile", submit)
        self.assertIn("local-notary", submit)
        self.assertNotIn("--password", submit)
        self.assertEqual(self.calls("xcrun")[-2:], [
            ["xcrun", "stapler", "staple", str(app)],
            ["xcrun", "stapler", "validate", str(app)],
        ])
        self.assertEqual(self.calls("spctl")[0][-1], str(app))
        self.assertIn("execute", self.calls("spctl")[0])

    def test_rejected_app_is_never_stapled(self):
        app = self.directory / "Muxy Beta.app"
        app.mkdir()
        self.env["NOTARY_STATUS"] = "Invalid"
        self.assertNotEqual(self.run_script("notarize-release.sh", app).returncode, 0)
        self.assertFalse(any(call[1] == "stapler" for call in self.calls("xcrun")))

    def test_zip_checks_notarized_binaries_without_stapling(self):
        archive = self.directory / "standalone.zip"
        with zipfile.ZipFile(archive, "w") as zipped:
            for name in ("muxy", "muxy-server"):
                zipped.writestr(name, '#!/bin/sh\necho build-info\n')
        result = self.run_script("notarize-release.sh", archive)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any(call[1] == "stapler" for call in self.calls("xcrun")))
        self.assertEqual(self.calls("spctl"), [])
        self.assertEqual(sum("--check-notarization" in call for call in self.calls("codesign")), 2)
        self.env["CODESIGN_EXIT"] = "1"
        self.assertNotEqual(self.run_script("notarize-release.sh", archive).returncode, 0)

    def test_rejected_notarization_with_zero_exit_is_not_stapled(self):
        self.env["NOTARY_STATUS"] = "Invalid"
        self.assertNotEqual(self.notarize().returncode, 0)
        self.assertEqual([call[1] for call in self.calls("xcrun")], ["notarytool", "notarytool"])
        self.assertEqual(self.calls("spctl"), [])

    def test_failed_submission_is_not_stapled(self):
        self.env["NOTARY_EXIT"] = "1"
        self.assertNotEqual(self.notarize().returncode, 0)
        self.assertEqual(self.calls("spctl"), [])

    def test_notarization_recovers_from_a_network_error(self):
        self.env["NOTARY_NETWORK_FAILURES"] = "1"
        result = self.notarize()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(sum(call[1:3] == ["notarytool", "submit"] for call in self.calls("xcrun")), 2)
        self.assertEqual(self.calls("sleep"), [["sleep", "5"]])
        self.assertEqual(len(self.calls("spctl")), 1)

    def test_notarization_network_retries_are_bounded(self):
        self.env["NOTARY_NETWORK_FAILURES"] = "3"
        result = self.notarize()
        self.assertEqual(result.returncode, 7, result.stderr)
        self.assertEqual([call[1:3] for call in self.calls("xcrun")], [["notarytool", "submit"]] * 3)
        self.assertEqual(len(self.calls("sleep")), 2)
        self.assertEqual(self.calls("spctl"), [])
        self.assertNotIn("JSONDecodeError", result.stderr)

    def test_notarization_auth_failure_is_not_retried(self):
        self.env["NOTARY_AUTH_FAILURE"] = "1"
        result = self.notarize()
        self.assertEqual(result.returncode, 9, result.stderr)
        self.assertEqual([call[1:3] for call in self.calls("xcrun")], [["notarytool", "submit"]])
        self.assertEqual(self.calls("sleep"), [])
        self.assertEqual(self.calls("spctl"), [])
        self.assertNotIn("JSONDecodeError", result.stderr)

    def test_gatekeeper_failure_fails_notarization_step(self):
        self.env["SPCTL_EXIT"] = "1"
        self.assertNotEqual(self.notarize().returncode, 0)


if __name__ == "__main__":
    unittest.main()
