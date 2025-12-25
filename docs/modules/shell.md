# shell - Execute Shell Commands

## Synopsis

The `shell` module takes a command and runs it through a shell (`/bin/sh`). This allows use of shell features like environment variables, pipes, redirects, and command chaining.

If you do not need shell features, use the `command` module instead as it is more secure.

## Classification

**RemoteCommand** - This module executes commands on remote hosts via SSH.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `cmd` | yes | - | string | The shell command to run. |
| `chdir` | no | - | string | Change into this directory before running the command. |
| `creates` | no | - | string | A filename or glob pattern. If it exists, this step will not run. |
| `removes` | no | - | string | A filename or glob pattern. If it does NOT exist, this step will not run. |
| `stdin` | no | - | string | Set stdin of the command directly to the specified value. |
| `executable` | no | /bin/sh | string | The shell to use for running the command. |

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

### Run a command with pipes

```yaml
- name: Get the number of running processes
  shell:
    cmd: ps aux | wc -l
```

### Use environment variables

```yaml
- name: Print home directory
  shell:
    cmd: echo $HOME
```

### Run a complex command with redirects

```yaml
- name: Backup configuration
  shell:
    cmd: tar czf /backup/config.tar.gz /etc/myapp 2>/dev/null
```

### Chain multiple commands

```yaml
- name: Update and clean up
  shell:
    cmd: apt-get update && apt-get autoremove -y
```

### Use a different shell

```yaml
- name: Run a bash-specific command
  shell:
    cmd: echo "Array: ${MY_ARRAY[@]}"
    executable: /bin/bash
```

### Conditional execution with creates

```yaml
- name: Generate SSL certificate only if not exists
  shell:
    cmd: openssl req -x509 -nodes -newkey rsa:4096 -keyout /etc/ssl/private/server.key -out /etc/ssl/certs/server.crt -days 365 -subj "/CN=myserver"
    creates: /etc/ssl/certs/server.crt
```

## Notes

- The `shell` module uses `/bin/sh` by default
- Shell commands are vulnerable to injection if not properly escaped
- Use the `command` module when shell features are not needed
- The command is marked as `changed` when it runs successfully
- In check mode, the command is not executed but would report as changed

## Security Warning

Be careful when constructing shell commands from variables. Always validate and sanitize user input to prevent command injection.

## See Also

- [command](command.md) - Execute commands without shell processing
