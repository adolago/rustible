# Alpha execution safety changes

The earlier executor reported simulated success for command, shell, package,
service and several filesystem modules. It could also run a remote-target file
task against the controller. These behaviors were defects, not supported modes.

## Before and after

- Native modules now run through the module registry. A successful command means
  the process ran; its output and exit status are preserved in registered results.
- A target executes locally only when inventory explicitly sets
  `ansible_connection: local`, or a library-created implicit localhost has no
  connection setting. A remote target without a connection fails as unreachable.
- Remote filesystem modules without a reviewed transport fail before changing
  controller files. A connection object alone does not establish that a module
  supports remote work. This release's CLI does not yet configure remote
  connections; it refuses remote native work instead of falling back locally.
- Modules unavailable in the registry fail explicitly. The previously simulated
  Python fallback is disabled until actual execution is implemented and tested.
- `--limit`, `--tags`, `--skip-tags`, and `--start-at-task` select the live schedule.
  Missing explicit inventories and unknown start tasks fail instead of reporting
  an empty successful run.
- Loop items execute separately. The previous batching path rewrote command
  arguments and fabricated identical per-item results; it is disabled until
  equivalent behavior is verified. Earlier batching performance claims therefore
  do not apply to this execution path.
- Handler notifications are scoped to the notifying host and current play.
  Handlers execute in definition order and their failures affect the final exit.
- Unrescued block failures stop ordinary following tasks. Retry settings are
  retained by the executor's parser.

## Confidentiality guard

End-to-end `no_log` redaction is not implemented across all logs, callbacks and
execution bundles. Playbooks and included task files containing an enabled
`no_log` directive now fail before task execution, without printing task content.
Only literal `no_log: false` is accepted. This is an intentional breaking safety
guard. Do not remove `no_log` to run a task containing secrets. Keep that workflow
on a tool with verified redaction until Rustible implements it completely.

## Migration and compatibility

For intended controller-local work, use an explicit local inventory entry. For
remote workflows, retain the remote identity: do not relabel a remote machine as
local to bypass a transport error. Unsupported meta actions and module privilege
escalation fail explicitly rather than claiming to have applied a control.
Local privilege escalation and play/config-wide escalation are currently
rejected; handler-specific transport/escalation directives are also rejected.
Local `get_url` is rejected until its destination write path is implemented.
Authored play/task connection settings are honored; remote facts and registered
results cannot change the transport into controller-local execution.

Review automation that treated exit zero as deployment evidence. Re-run in a
disposable environment and verify actual effects before production use. The
focused `diligence_execution_tests` exercise CLI selection, real command results,
controller file protection, confidentiality rejection, blocks, retries and
handlers. They do not verify real SSH, cloud, Windows or physical HPC systems.

## Test execution safety

Do not run the broad legacy executor/scenario suite on a developer machine after
this change. Those tests were written against partly simulated execution and are
not established as controller-safe. This is a source-review warning, not a claim
that those commands have been run during diligence.

`tests/scenario_tests.rs` contains package installation and service start/stop
tasks for nginx, PostgreSQL, PHP-FPM, HAProxy, node exporter and application
services. If executed on a local target, registry dispatch can reach `apt-get
install/remove/update/upgrade`, other platform package-manager equivalents, and
`systemctl start/stop/restart` or the platform service manager. User/group modules
can reach `useradd/usermod/userdel` and `groupadd/groupmod/groupdel`. Shell and
command tasks run their actual program strings; they are not a dry-run mechanism.

Some tests already contain controller paths: for example
`test_handler_access_to_registered_variables` in `tests/handler_tests.rs` runs
`echo 'ready'` and a copy to `/etc/test.conf`. Other fixtures use paths under
`/etc/nginx`. A missing source file or current lack of root access is not a safety
boundary. This inventory of reachable effects is intentionally conservative;
some similarly named tests only parse or construct tasks and never execute them.

Run broad tests only in disposable containers, or an equivalent sandbox with a
read-only host root, separate writable temporary directories, isolated `/run`
and home directories, no credentials, and no network. On the host, restrict
execution to the explicitly bounded `diligence_execution_tests` and
`executor::task::tests::diligence_` tests. Compilation and formatting do not run
playbook tasks, but build scripts remain part of the dependency trust boundary.
