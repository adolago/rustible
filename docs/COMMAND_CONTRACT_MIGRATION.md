# Local command and shell construction corrections

These changes apply to the Unix control node and do not establish native Windows
support, complete command safety or remote execution equivalence.

## Command arguments

Local `command` now parses `cmd` and `_raw_params` with the existing `shell-words`
dependency. POSIX single/double quotes group arguments and are removed during
parsing; an empty quoted argument remains an empty argument. Previously, splitting
only on whitespace broke quoted groups and passed quote characters to the child.
Unclosed quotes and an empty program are errors before any process is spawned.

Explicit `argv` still takes precedence and its elements remain literal, including
empty arguments after the program. Its program must be nonempty. No shell,
variable expansion, wildcard expansion or command substitution is added to local
command execution. Existing registry validation of command text is unchanged;
it still rejects its listed shell operators and backslash syntax. Use `argv` to
state literal argument boundaries directly.

Local check mode now constructs and validates the process without spawning it,
when no guard has already skipped the task. Invalid quoting or environment names may
therefore be rejected in check mode where they previously reached only apply.
Remote free-form command strings and their existing escaping/validation path are
unchanged by the local parser repair.

## Creates and removes guards

Local `command` and `shell` resolve relative `creates` and `removes` paths from the
same directory used by the child: explicit `chdir`, then `context.work_dir`, then
the process's current directory. An explicit `chdir` overrides the context rather
than being joined to it. Relative directory settings themselves remain relative
to the process's current directory. Absolute guard paths remain absolute.

The guard is still checked before execution or check-mode preview. This does not
add glob support, prevent a concurrent path change, or repair the existing
filesystem-error/absence distinction. Remote guards and transport working
directory behavior require separate review.

## Shell executable arguments

Local `shell.executable` now supplies a parsed program and argument list. For
example, `/usr/bin/env sh` constructs program `/usr/bin/env`, argument `sh`, then
the module's command flag and the original command body. Quoted program paths
and arguments are preserved as individual values. The command body remains one
opaque argument; this change does not parse or rewrite shell-language commands.

Local construction, remote POSIX construction and executable validation use the
same parser. Empty or malformed executables and explicit managed command flags
(`-c`, `/c`, `/C`) are rejected by the builders, including direct library calls.
This keeps the existing executable restrictions when local argument support is
enabled. Existing command-flag selection and remote cmd.exe escaping are retained;
this is not a Windows-shell or wrapper portability certification.

## Remaining limits and verification

MOD018 remains partial: local timeouts and privilege escalation are not repaired.
MOD019 is unchanged: synchronous stdin writing can deadlock with captured output,
and remote stdin is not implemented. No process pumping, cancellation, output
limit or new process-execution mechanism is introduced here. Those contracts need
separate fixes before broad command-execution claims can be made.

Focused regressions inspect process builders and use check mode with temporary
guard files. They require no executable program and spawn no child process. This
proves construction and guard decisions, not the effects of a real local or
remote command.
