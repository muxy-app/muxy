import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
NETWORK_ERROR = '68 error is Error Domain=NSURLErrorDomain Code=-1005 "The network connection was lost."'
BLOCKED_NETWORK_ERROR = '68 error is Error Domain=NSPOSIXErrorDomain Code=1 "Operation not permitted"'
OTHER_ERROR = '65 CloudKit query failed due to "Record Not Found".'
XCRUN = """
if [[ "$1" != stapler ]]; then
    [[ "$2" != submit ]] || echo '{"id":"submission","status":"Accepted"}'
    exit 0
fi
echo "stapler $2" >> "$STUB_CALLS"
OUTCOME="$(head -n 1 "$STUB_OUTCOMES")"
tail -n +2 "$STUB_OUTCOMES" > "$STUB_OUTCOMES.next"
mv "$STUB_OUTCOMES.next" "$STUB_OUTCOMES"
[[ -n "$OUTCOME" ]] || exit 0
echo "${OUTCOME#* }" >&2
exit "${OUTCOME%% *}"
"""
STUBS = {
    "xcrun": XCRUN,
    "sleep": 'echo "sleep $1" >> "$STUB_CALLS"',
    "spctl": 'echo spctl >> "$STUB_CALLS"',
    "codesign": 'echo codesign >> "$STUB_CALLS"',
    "ditto": ":",
}


class NotarizeReleaseTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.stubs = self.directory / "stubs"
        self.stubs.mkdir()
        for name, body in STUBS.items():
            stub = self.stubs / name
            stub.write_text(f"#!/bin/bash\n{body}\n")
            stub.chmod(0o755)

    def notarize(self, outcomes, artifact_name="Muxy.dmg"):
        artifact = self.directory / artifact_name
        if artifact_name.endswith(".app"):
            artifact.mkdir()
        else:
            artifact.touch()
        calls = self.directory / f"{artifact_name}.calls"
        calls.touch()
        planned = self.directory / f"{artifact_name}.outcomes"
        planned.write_text("".join(f"{outcome}\n" for outcome in outcomes))
        result = subprocess.run(
            ["bash", ROOT / "scripts/notarize-release.sh", artifact],
            env={
                **os.environ,
                "PATH": f"{self.stubs}{os.pathsep}{os.environ['PATH']}",
                "NOTARY_KEYCHAIN_PROFILE": "test",
                "STUB_CALLS": str(calls),
                "STUB_OUTCOMES": str(planned),
            },
            capture_output=True,
            text=True,
            timeout=30,
        )
        return result, calls.read_text().splitlines()

    def test_network_errors_while_stapling_retry_with_growing_delays(self):
        result, calls = self.notarize([NETWORK_ERROR, NETWORK_ERROR])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls, [
            "stapler staple", "sleep 10",
            "stapler staple", "sleep 20",
            "stapler staple",
            "stapler validate",
            "spctl",
        ])

    def test_network_failures_retry_whichever_error_domain_reports_them(self):
        result, calls = self.notarize([BLOCKED_NETWORK_ERROR])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls, [
            "stapler staple", "sleep 10",
            "stapler staple",
            "stapler validate",
            "spctl",
        ])

    def test_a_network_error_while_validating_does_not_staple_again(self):
        result, calls = self.notarize(["", NETWORK_ERROR])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls, [
            "stapler staple",
            "stapler validate", "sleep 10",
            "stapler validate",
            "spctl",
        ])

    def test_app_bundles_retry_network_errors_too(self):
        result, calls = self.notarize([NETWORK_ERROR, "", NETWORK_ERROR], "Muxy.app")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls, [
            "stapler staple", "sleep 10",
            "stapler staple",
            "stapler validate", "sleep 10",
            "stapler validate",
            "codesign",
            "spctl",
        ])

    def test_a_network_outage_fails_after_the_last_attempt(self):
        result, calls = self.notarize([NETWORK_ERROR] * 7)
        self.assertEqual(result.returncode, 68)
        self.assertEqual(calls, [
            "stapler staple", "sleep 10",
            "stapler staple", "sleep 20",
            "stapler staple", "sleep 40",
            "stapler staple", "sleep 80",
            "stapler staple", "sleep 160",
            "stapler staple",
        ])

    def test_other_stapler_failures_stop_immediately_and_stay_visible(self):
        result, calls = self.notarize([OTHER_ERROR])
        self.assertEqual(result.returncode, 65)
        self.assertEqual(calls, ["stapler staple"])
        self.assertIn("Record Not Found", result.stdout + result.stderr)

    def test_a_failure_in_any_stapler_call_skips_the_later_checks(self):
        for extension in ["dmg", "app"]:
            for command, earlier_outcomes in [("staple", []), ("validate", [""])]:
                for failure, status, runs in [(NETWORK_ERROR, 68, 6), (OTHER_ERROR, 65, 1)]:
                    with self.subTest(extension=extension, command=command, status=status):
                        artifact = f"{command}-{status}.{extension}"
                        result, calls = self.notarize(earlier_outcomes + [failure] * 7, artifact)
                        self.assertEqual(result.returncode, status)
                        self.assertEqual(
                            [call for call in calls if not call.startswith("sleep")],
                            ["stapler staple"] * len(earlier_outcomes) + [f"stapler {command}"] * runs,
                        )


if __name__ == "__main__":
    unittest.main()
