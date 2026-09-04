#!/usr/bin/env python3
"""Exercise the comparison entrypoint using mock executables and temporary data.

No existing inventory/playbook is copied or executed. Cargo, Ansible and Rustible
are all inert local stubs; the fixture directory deliberately contains spaces.
"""

import csv
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
PLAYBOOKS = [
    "bench_01_simple.yml", "bench_02_file_ops.yml", "bench_03_multi_task.yml",
    "bench_04_comprehensive.yml", "bench_05_many_hosts.yml", "bench_06_many_tasks.yml",
    "bench_07_templates.yml", "bench_08_loops.yml", "bench_09_handlers.yml",
    "bench_10_conditionals.yml",
]
STUB = r'''#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys

tool = Path(sys.argv[0]).name
args = sys.argv[1:]
calls = Path(os.environ["MOCK_CALLS"])
previous = [json.loads(line) for line in calls.read_text().splitlines()] if calls.exists() else []
with calls.open("a") as output:
    output.write(json.dumps({"tool": tool, "args": args}) + "\n")
if tool == "cargo":
    if args and args[0] == "metadata":
        print(json.dumps({"target_directory": os.environ["MOCK_TARGET"]}))
    elif args == ["--version"]:
        print("cargo synthetic-version")
    else:
        print("synthetic build diagnostics")
        sys.exit(int(os.environ.get("MOCK_BUILD_EXIT", "0")))
elif args == ["--version"]:
    print(tool + " synthetic-version")
else:
    occurrence = 1 + sum(call["tool"] == tool and call["args"] != ["--version"] for call in previous)
    if tool == os.environ.get("MOCK_FAIL_TOOL") and occurrence == int(os.environ.get("MOCK_FAIL_CALL", "0")):
        print("synthetic failure diagnostics", file=sys.stderr)
        sys.exit(42)
    print("synthetic invocation succeeded")
'''


class ComparisonRunnerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="benchmark test ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.comparison = self.root / "benches" / "comparison"
        self.comparison.mkdir(parents=True)
        for name in ["run_benchmark.sh", "run_benchmark.py"]:
            source = ROOT / "benches" / "comparison" / name
            if source.exists():
                shutil.copy2(source, self.comparison / name)
        self.inventory = self.comparison / "inventory.yml"
        self.inventory.write_text("all:\n  hosts:\n    synthetic:\n      ansible_connection: local\n")
        for name in PLAYBOOKS:
            (self.comparison / name).write_text("- hosts: synthetic\n  tasks: []\n")
        self.mock_bin = self.root / "mock tools"
        self.mock_bin.mkdir()
        self.target = self.root / "target"
        (self.target / "release").mkdir(parents=True)
        for path in [self.mock_bin / "cargo", self.mock_bin / "ansible-playbook",
                     self.target / "release" / "rustible"]:
            path.write_text(STUB)
            path.chmod(0o700)
        # The old script pauses between runs; keep the baseline regression bounded.
        for name in ["sleep", "bc"]:
            (self.mock_bin / name).write_text("#!/bin/sh\nexit 0\n")
            (self.mock_bin / name).chmod(0o700)
        self.calls_file = self.root / "calls.jsonl"

    def run_runner(self, **overrides):
        env = os.environ.copy()
        env.update({
            "PATH": str(self.mock_bin) + os.pathsep + env.get("PATH", ""),
            "RUNS": "1", "MOCK_CALLS": str(self.calls_file),
            "MOCK_TARGET": str(self.target),
        })
        env.update(overrides)
        return subprocess.run(
            ["bash", str(self.comparison / "run_benchmark.sh")],
            cwd=self.root, env=env, capture_output=True, text=True, timeout=30,
        )

    def calls(self):
        if not self.calls_file.exists():
            return []
        return [json.loads(line) for line in self.calls_file.read_text().splitlines()]

    def result_directory(self):
        files = list((self.comparison / "results").glob("run_*/invocations.csv"))
        self.assertEqual(len(files), 1, "one private run directory must contain raw invocations")
        return files[0].parent

    def rows(self, directory):
        with (directory / "invocations.csv").open(newline="") as source:
            return list(csv.DictReader(source))

    def test_success_records_raw_invocations_without_equivalence_claims(self):
        result = self.run_runner(RUNS="2")
        self.assertEqual(result.returncode, 0, result.stderr)
        directory = self.result_directory()
        rows = self.rows(directory)
        self.assertEqual(len(rows), 42)
        self.assertEqual(sum(row["phase"] == "warmup" for row in rows), 2)
        self.assertEqual(sum(row["phase"] == "measured" for row in rows), 40)
        self.assertTrue(all(row["exit_code"] == "0" for row in rows))
        self.assertTrue(all(float(row["duration_ms"]) >= 0 for row in rows))
        self.assertNotIn("hosts", rows[0])
        self.assertNotIn("tasks", rows[0])
        self.assertEqual({row["playbook"] for row in rows}, set(PLAYBOOKS))
        metadata = json.loads((directory / "metadata.json").read_text())
        self.assertEqual(metadata["status"], "complete")
        self.assertEqual(metadata["effect_equivalence"], "unverified")
        self.assertEqual(metadata["inventory_sha256"], hashlib.sha256(self.inventory.read_bytes()).hexdigest())
        self.assertEqual(set(metadata["playbook_sha256"]), set(PLAYBOOKS))
        self.assertIn("synthetic-version", metadata["versions"]["rustible"])
        summary = (directory / "summary.txt").read_text().lower()
        self.assertIn("median", summary)
        self.assertIn("unverified", summary)
        self.assertNotIn("speedup", summary)
        self.assertNotIn("synthetic invocation succeeded", result.stdout + result.stderr)
        self.assertEqual(stat.S_IMODE(directory.stat().st_mode), 0o700)
        for path in directory.iterdir():
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600, path.name)
        invocations = [call for call in self.calls() if call["tool"] == "rustible" and call["args"] != ["--version"]]
        self.assertEqual(len(invocations), 21)
        self.assertEqual(invocations[0]["args"], ["run", str(self.comparison / PLAYBOOKS[0]), "-i", str(self.inventory)])

    def check_failure(self, tool, call, phase):
        result = self.run_runner(MOCK_FAIL_TOOL=tool, MOCK_FAIL_CALL=str(call))
        self.assertEqual(result.returncode, 42, result.stderr)
        directory = self.result_directory()
        rows = self.rows(directory)
        self.assertEqual(rows[-1]["exit_code"], "42")
        self.assertEqual(rows[-1]["phase"], phase)
        self.assertEqual(rows[-1]["tool"], "ansible" if tool == "ansible-playbook" else tool)
        self.assertEqual(sum(row["exit_code"] != "0" for row in rows), 1)
        metadata = json.loads((directory / "metadata.json").read_text())
        self.assertEqual(metadata["status"], "failed")
        self.assertIn("INCOMPLETE", (directory / "summary.txt").read_text())
        self.assertNotIn("median", (directory / "summary.txt").read_text().lower())
        self.assertNotIn("synthetic failure diagnostics", result.stdout + result.stderr)
        logs = list(directory.glob("*.log"))
        self.assertTrue(any("synthetic failure diagnostics" in path.read_text() for path in logs))
        tool_calls = [entry for entry in self.calls() if entry["tool"] == tool and entry["args"] != ["--version"]]
        self.assertEqual(len(tool_calls), call, "failed invocation must stop the run")

    def test_ansible_warmup_failure_stops_run(self):
        self.check_failure("ansible-playbook", 1, "warmup")

    def test_rustible_warmup_failure_stops_run(self):
        self.check_failure("rustible", 1, "warmup")

    def test_ansible_measured_failure_stops_run(self):
        self.check_failure("ansible-playbook", 2, "measured")

    def test_rustible_measured_failure_stops_run(self):
        self.check_failure("rustible", 2, "measured")

    def test_build_failure_retains_diagnostic_and_runs_no_tools(self):
        result = self.run_runner(MOCK_BUILD_EXIT="31")
        self.assertEqual(result.returncode, 31)
        self.assertFalse(any(call["tool"] != "cargo" for call in self.calls()))
        logs = list((self.comparison / "results").glob("run_*/build.log"))
        self.assertEqual(len(logs), 1)
        self.assertIn("synthetic build diagnostics", logs[0].read_text())
        self.assertNotIn("synthetic build diagnostics", result.stdout + result.stderr)

    def test_invalid_run_counts_fail_before_any_tool_is_invoked(self):
        for value in ["0", "-2", "not-a-number", "1.5", "10001"]:
            with self.subTest(value=value):
                result = self.run_runner(RUNS=value)
                self.assertEqual(result.returncode, 2)
                self.assertEqual(self.calls(), [])


if __name__ == "__main__":
    unittest.main()
