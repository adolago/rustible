#!/usr/bin/env python3
"""Record invocation timings; successful exits do not prove equivalent effects."""

import csv
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time


PLAYBOOKS = (
    "bench_01_simple.yml", "bench_02_file_ops.yml", "bench_03_multi_task.yml",
    "bench_04_comprehensive.yml", "bench_05_many_hosts.yml", "bench_06_many_tasks.yml",
    "bench_07_templates.yml", "bench_08_loops.yml", "bench_09_handlers.yml",
    "bench_10_conditionals.yml",
)
FIELDS = ("tool", "playbook", "phase", "run", "duration_ms", "exit_code")


class RunFailure(Exception):
    def __init__(self, stage, code):
        super().__init__(f"{stage} failed with exit code {code}")
        self.code = 128 - code if code < 0 else code


def file_hash(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run_logged(command, log_path, cwd):
    """Keep all child output in a private file; never interpret command as shell."""
    with log_path.open("wb") as output:
        try:
            return subprocess.run(command, cwd=cwd, stdout=output, stderr=subprocess.STDOUT).returncode
        except OSError as error:
            output.write(str(error).encode("utf-8", errors="replace"))
            return 127


def checked(command, log_path, cwd, stage):
    code = run_logged(command, log_path, cwd)
    if code:
        raise RunFailure(stage, code)


def measure(tool, command, playbook, phase, number, directory, cwd, writer, rows):
    log_path = directory / f"{phase}-{tool}-{Path(playbook).stem}-{number}.log"
    start = time.perf_counter_ns()
    code = run_logged(command, log_path, cwd)
    duration_ms = (time.perf_counter_ns() - start) / 1_000_000
    row = dict(zip(FIELDS, (tool, playbook, phase, number, f"{duration_ms:.6f}", code)))
    writer.writerow(row)
    rows.append(row)
    if code:
        raise RunFailure(f"{phase} {tool} {playbook} run {number}", code)


def summary_text(metadata, rows):
    lines = ["Raw process invocation timings", "Effect equivalence: unverified."]
    if metadata["status"] != "complete":
        lines.extend(["INCOMPLETE: no aggregate timing summary.", metadata.get("failure", "Run did not complete")])
    else:
        lines.append("Measured durations in milliseconds; warm-ups excluded.")
        lines.append("tool,playbook,n,min,median,max")
        for playbook in PLAYBOOKS:
            for tool in ("ansible", "rustible"):
                durations = [float(row["duration_ms"]) for row in rows
                             if row["phase"] == "measured" and row["playbook"] == playbook
                             and row["tool"] == tool]
                lines.append(f"{tool},{playbook},{len(durations)},{min(durations):.6f},"
                             f"{statistics.median(durations):.6f},{max(durations):.6f}")
    lines.append("Host/task execution counts, target resets and equal effects are not verified.")
    return "\n".join(lines) + "\n"


def main():
    runs_text = os.environ.get("RUNS", "5")
    if len(sys.argv) != 1 or not re.fullmatch(r"[1-9][0-9]{0,4}", runs_text) or int(runs_text) > 10000:
        print("Usage: RUNS=1..10000 ./run_benchmark.sh (no positional arguments)", file=sys.stderr)
        return 2
    runs = int(runs_text)
    os.umask(0o077)
    comparison = Path(__file__).resolve().parent
    project = comparison.parent.parent
    inventory = comparison / "inventory.yml"
    results = comparison / "results"
    results.mkdir(parents=True, exist_ok=True)
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
    directory = Path(tempfile.mkdtemp(prefix=f"run_{timestamp}_", dir=results))
    metadata = {
        "schema_version": 2, "status": "incomplete", "effect_equivalence": "unverified",
        "runs_per_playbook": runs, "started_at": datetime.now(timezone.utc).isoformat(),
        "measurement": "monotonic elapsed time around each process invocation",
        "platform": platform.platform(), "cpu_count": os.cpu_count(),
        "python_version": platform.python_version(), "versions": {},
        "tool_order": ["ansible", "rustible"], "target_reset_between_tools": False,
        "warmup": {"playbook": PLAYBOOKS[0], "invocations_per_tool": 1},
    }
    rows = []
    exit_code = 0
    try:
        metadata["inventory_sha256"] = file_hash(inventory)
        metadata["playbook_sha256"] = {name: file_hash(comparison / name) for name in PLAYBOOKS}
        metadata["runner_sha256"] = {name: file_hash(comparison / name)
                                      for name in ("run_benchmark.sh", "run_benchmark.py")}
        cargo = shutil.which("cargo")
        ansible = shutil.which("ansible-playbook")
        if not cargo or not ansible:
            raise RunFailure("required cargo/ansible-playbook executable lookup", 127)
        checked([cargo, "build", "--locked", "--release"], directory / "build.log", project, "build")
        cargo_metadata = directory / "cargo-metadata.log"
        checked([cargo, "metadata", "--no-deps", "--format-version", "1"], cargo_metadata, project, "Cargo metadata")
        target = Path(json.loads(cargo_metadata.read_text())["target_directory"])
        rustible = target / "release" / "rustible"
        if not rustible.is_file() or not os.access(rustible, os.X_OK):
            raise RunFailure("built Rustible executable lookup", 127)
        metadata["rustible_binary_sha256"] = file_hash(rustible)
        for tool, executable in (("cargo", cargo), ("ansible", ansible), ("rustible", str(rustible))):
            log_path = directory / f"version-{tool}.log"
            checked([executable, "--version"], log_path, project, f"{tool} version")
            metadata["versions"][tool] = log_path.read_text(errors="replace").strip()
        if shutil.which("git"):
            commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=project, capture_output=True, text=True)
            metadata["git_commit"] = commit.stdout.strip() if commit.returncode == 0 else None
            status = subprocess.run(["git", "status", "--porcelain", "--untracked-files=no"],
                                    cwd=project, capture_output=True, text=True)
            metadata["tracked_source_dirty"] = bool(status.stdout) if status.returncode == 0 else None

        def commands(playbook):
            return (("ansible", [ansible, "-i", str(inventory), str(comparison / playbook)]),
                    ("rustible", [str(rustible), "run", str(comparison / playbook), "-i", str(inventory)]))

        with (directory / "invocations.csv").open("w", newline="") as output:
            writer = csv.DictWriter(output, fieldnames=FIELDS)
            writer.writeheader()
            for tool, command in commands(PLAYBOOKS[0]):
                measure(tool, command, PLAYBOOKS[0], "warmup", 1, directory, project, writer, rows)
                output.flush()
            for playbook in PLAYBOOKS:
                for number in range(1, runs + 1):
                    for tool, command in commands(playbook):
                        measure(tool, command, playbook, "measured", number, directory, project, writer, rows)
                        output.flush()
        metadata["status"] = "complete"
    except RunFailure as error:
        metadata.update(status="failed", failure=str(error))
        exit_code = error.code
    except KeyboardInterrupt:
        metadata.update(status="failed", failure="Timing run interrupted")
        exit_code = 130
    except (OSError, ValueError, KeyError, TypeError):
        metadata.update(status="failed", failure="Runner setup or artifact processing failed")
        exit_code = 1
    finally:
        metadata["finished_at"] = datetime.now(timezone.utc).isoformat()
        (directory / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
        (directory / "summary.txt").write_text(summary_text(metadata, rows))
    label = "Timing run complete; effect equivalence unverified" if not exit_code else "Timing run failed"
    print(f"{label}. Private results: {directory}")
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
