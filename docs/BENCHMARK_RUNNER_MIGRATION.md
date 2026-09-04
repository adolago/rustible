# Comparison runner evidence contract

`benches/comparison/run_benchmark.sh` retains its entrypoint and the same ten
playbook fixtures, but delegates to a Python 3 standard-library helper. It now
records raw process invocation timings. It does not calculate speedup ratios
or claim that Ansible and Rustible performed equivalent work.

The runner still executes real playbooks against its configured inventory.
Use disposable, isolated benchmark targets: the supplied fixtures create and
remove files, execute commands and depend on target state. This patch was
checked only with mock executables and temporary synthetic fixtures; none of
the supplied remote inventory or playbooks was executed for validation.

## Failure and output changes

Every build, version check, initial warm-up and measured invocation must exit
successfully. The first failure stops the run and returns its nonzero exit
status (a terminated child is mapped to the usual 128 plus signal status).
Failed invocations remain in the CSV with their exit code and duration; their
run receives an INCOMPLETE summary without aggregate timings. Output from the
tools remains in private per-invocation logs, including failed warm-ups.

Each run creates a unique `results/run_<UTC time>_<suffix>/` directory with
mode 0700 and files with mode 0600. Console messages show the status and run
directory, never child log contents. Treat these logs as private because real
playbooks may print sensitive values. Older result files are preserved.

The previous CSV columns `hosts` and `tasks` have been removed. Counting source
lines containing `name:` counted play names, variable data and handlers, not
executed tasks; a hard-coded host count was not runtime evidence. The new
`invocations.csv` columns are `tool,playbook,phase,run,duration_ms,exit_code`.
`phase` distinguishes the single initial simple-playbook warm-up from measured
invocations. `summary.txt` reports count, minimum, median and maximum measured
duration for each tool/playbook separately; warm-ups are excluded.

`metadata.json` records completion status, UTC timestamps, runner/input hashes,
the built Rustible binary hash, reported tool versions, local platform details,
Git commit/dirty state where available, invocation order and the absence of
target resets. Inventory contents and addresses are not copied into metadata.
The locked release build is outside the timed interval. Cargo metadata locates
the actual target directory; there is no timed fallback through `cargo run`.
Python's monotonic clock measures elapsed time around each child invocation.
`RUNS` must be an integer from 1 through 10000; the default remains 5.

## Evidence limits

Exit success is necessary, but it does not establish host/task coverage or
equal effects. The ten fixtures include loops, conditionals, handlers, variable
rendering, multiple plays and cleanup. Several commands expose changing host
data, and cleanup can erase observable effects before a comparison. No common
postcondition verifier exists for the whole set.

The tools still run in a fixed Ansible-then-Rustible order against shared state.
There is no reset, randomized order, equivalent-output comparison or persistent
SSH-session guarantee from the initial warm-up. Raw timing differences remain
subject to those limitations. PROCESS-002 is only partially repaired until
equivalent effects and a controlled comparison are demonstrated. Historical
CSV values and published performance claims are not revalidated by this patch.
