# command - Execute Commands

## Synopsis

The `command` module executes commands on remote hosts. Unlike the `shell` module, it does not process commands through a shell, so variables like `$HOME` and operations like `<`, `>`, `|`, `;` and `&` will not work.

Use the `shell` module if you need those features.

## Classification

**RemoteCommand** - This module executes commands on remote hosts via SSH.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `cmd` | yes* | - | string | The command to run. Either this or `argv` is required. |
| `argv` | yes* | - | list | Pass the command as a list rather than a string. |
| `chdir` | no | - | string | Change into this directory before running the command. |
| `creates` | no | - | string | A filename or glob pattern. If it exists, this step will not run. |
| `removes` | no | - | string | A filename or glob pattern. If it does NOT exist, this step will not run. |
| `stdin` | no | - | string | Set stdin of the command directly to the specified value. |

*Either `cmd` or `argv` must be provided.

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `cmd` | string | The command that was executed |
| `stdout` | string | The standard output of the command |
| `stderr` | string | The standard error output of the command |
| `rc` | integer | The return code of the command |
| `start` | string | The timestamp when the command started |
| `end` | string | The timestamp when the command ended |
| `delta` | string | The time elapsed during command execution |

## Examples

### Run a simple command

```yaml
- name: Get uptime
  command:
    cmd: uptime
```

### Run a command with chdir

```yaml
- name: Run a command in a specific directory
  command:
    cmd: ls -la
    chdir: /var/log
```

### Run a command only if a file does not exist

```yaml
- name: Initialize database only if not already done
  command:
    cmd: /usr/local/bin/init-db.sh
    creates: /var/lib/myapp/db_initialized
```

### Run a command only if a file exists

```yaml
- name: Clean up old logs if they exist
  command:
    cmd: rm -f /var/log/myapp/*.old
    removes: /var/log/myapp/*.old
```

### Use argv for commands with special characters

```yaml
- name: Echo a message with special characters
  command:
    argv:
      - echo
      - "Hello, World!"
```

## Notes

- The `command` module does not use a shell, so shell-specific syntax will not work
- For shell features like pipes, redirects, or environment variables, use the `shell` module
- The module is idempotent when using `creates` or `removes` parameters
- Return code 0 indicates success; any other code indicates failure
- The command is marked as `changed` when it runs successfully

## See Also

- [shell](shell.md) - Execute shell commands with full shell features
