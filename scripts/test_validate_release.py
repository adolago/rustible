"""Offline regression tests for the release validation gate."""

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from validate_release import validate_version


class ReleasePolicyTests(unittest.TestCase):
    def run_gate(self, manifest, release_env):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text(manifest, encoding="utf-8")
            output = root / "github-output"
            output.write_text("existing=value\n", encoding="utf-8")
            env = {
                "PATH": os.defpath,
                "GITHUB_OUTPUT": str(output),
                **release_env,
            }
            result = subprocess.run(
                [sys.executable, str(Path(__file__).with_name("validate_release.py").resolve())],
                cwd=root, env=env, capture_output=True, text=True, timeout=10,
                check=False,
            )
            return result, output.read_text(encoding="utf-8")

    def test_tag_entrypoint_appends_only_validated_outputs(self):
        result, output = self.run_gate(
            '[package]\nversion = "1.2.3-rc.1"\n',
            {"GITHUB_EVENT_NAME": "push", "GITHUB_REF": "refs/tags/v1.2.3-rc.1"},
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(output, "existing=value\nversion=1.2.3-rc.1\nis_prerelease=true\n")

    def test_dispatch_entrypoint_uses_input_not_branch_ref(self):
        result, output = self.run_gate(
            '[package]\nversion = "1.2.3+build.42"\n',
            {"GITHUB_EVENT_NAME": "workflow_dispatch", "GITHUB_REF": "refs/heads/main",
             "RELEASE_VERSION_INPUT": "1.2.3+build.42"},
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(output, "existing=value\nversion=1.2.3+build.42\nis_prerelease=false\n")

    def test_failed_entrypoints_never_append_output(self):
        manifest = '[package]\nversion = "1.2.3"\n'
        cases = [
            (manifest, {"GITHUB_EVENT_NAME": "push", "GITHUB_REF": "refs/heads/main"}),
            (manifest, {"GITHUB_EVENT_NAME": "push", "GITHUB_REF": "refs/tags/v1.2.4"}),
            (manifest, {"GITHUB_EVENT_NAME": "workflow_dispatch"}),
            (manifest, {"GITHUB_EVENT_NAME": "workflow_dispatch",
                        "RELEASE_VERSION_INPUT": "1.2.3\nis_prerelease=false"}),
            ('[package\n', {"GITHUB_EVENT_NAME": "push", "GITHUB_REF": "refs/tags/v1.2.3"}),
            ('[package]\nname = "test"\n',
             {"GITHUB_EVENT_NAME": "push", "GITHUB_REF": "refs/tags/v1.2.3"}),
        ]
        for index, (content, env) in enumerate(cases):
            with self.subTest(case=index):
                result, output = self.run_gate(content, env)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(output, "existing=value\n")

    def test_exact_versions_and_prerelease_status(self):
        for version, prerelease in [
            ("1.0.0", False), ("0.1.1-alpha", True),
            ("1.2.3-rc.1+build.42", True), ("1.2.3+build.42", False),
        ]:
            with self.subTest(version=version):
                self.assertEqual(validate_version(version, version), prerelease)

    def test_mismatch_including_prerelease_is_fatal(self):
        for release, package in [("1.0.0", "0.1.1-alpha"), ("1.0.0-beta", "1.0.0")]:
            with self.subTest(release=release):
                with self.assertRaises(ValueError):
                    validate_version(release, package)

    def test_invalid_and_output_injection_versions_are_rejected(self):
        for version in ["", "v1.0.0", "01.0.0", "1.0.0-01", "1.0.0-", "1.0.0\nversion=2.0.0", '1.0.0"']:
            with self.subTest(version=version):
                with self.assertRaises(ValueError):
                    validate_version(version, version)


if __name__ == "__main__":
    unittest.main()
