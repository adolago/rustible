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

## Real-World Use Cases

### Database Backup with Compression

```yaml
- name: Backup PostgreSQL database
  shell:
    cmd: pg_dump mydb | gzip > /backup/mydb_$(date +%Y%m%d).sql.gz
    creates: /backup/mydb_$(date +%Y%m%d).sql.gz
  become: yes
  become_user: postgres
```

### Log Rotation and Cleanup

```yaml
- name: Clean old logs
  shell:
    cmd: find /var/log/myapp -name "*.log" -mtime +30 -exec rm {} \; && echo "Cleaned"
  register: cleanup_result
  changed_when: "'Cleaned' in cleanup_result.stdout"
```

### Service Health Monitoring

```yaml
- name: Check if services are healthy
  shell:
    cmd: |
      for svc in nginx postgresql redis; do
        systemctl is-active $svc || exit 1
      done
  register: health_check
  failed_when: health_check.rc != 0
```

### Environment-Aware Commands

```yaml
- name: Run application with environment
  shell:
    cmd: source /opt/myapp/.env && /opt/myapp/bin/migrate
    executable: /bin/bash
  args:
    chdir: /opt/myapp
```

### Conditional Command Execution

```yaml
- name: Restart only if config changed
  shell:
    cmd: |
      md5sum /etc/nginx/nginx.conf > /tmp/nginx.md5.new
      if ! diff -q /tmp/nginx.md5 /tmp/nginx.md5.new > /dev/null 2>&1; then
        systemctl reload nginx
        mv /tmp/nginx.md5.new /tmp/nginx.md5
        echo "changed"
      fi
  register: nginx_reload
  changed_when: "'changed' in nginx_reload.stdout"
```

## Troubleshooting

### Command works interactively but fails in playbook

Shell sessions in playbooks do not have the same environment as interactive shells. Source environment files explicitly:

```yaml
- name: Run with full environment
  shell:
    cmd: source ~/.bashrc && mycommand
    executable: /bin/bash
```

### Pipe returns wrong exit code

By default, the shell returns the exit code of the last command in a pipe. Use `pipefail` to catch errors:

```yaml
- name: Fail if any pipe component fails
  shell:
    cmd: set -o pipefail && cat /etc/passwd | grep nonexistent | wc -l
    executable: /bin/bash
  register: result
  failed_when: result.rc != 0 and result.rc != 1
```

### Quotes and escaping issues

Use YAML literal blocks to avoid escaping issues:

```yaml
- name: Complex command with quotes
  shell:
    cmd: |
      echo "Hello $USER" | awk '{print $1}'
```

### Command hangs or times out

Long-running commands may need timeout handling:

```yaml
- name: Run with timeout
  shell:
    cmd: timeout 60 long_running_command || exit 1
```

### Variable expansion not working

Ensure you are using shell module (not command) and proper quoting:

```yaml
# CORRECT - shell variables expand
- shell:
    cmd: echo "Home is $HOME"

# Also CORRECT - Jinja2 variable
- shell:
    cmd: echo "User is {{ ansible_user }}"
```

### Script fails silently

Enable strict mode for better error detection:

```yaml
- name: Run with strict error checking
  shell:
    cmd: |
      set -euo pipefail
      command1
      command2
      command3
    executable: /bin/bash
```

### Environment variables from become_user not loaded

When using become, the target user's environment is not fully loaded:

```yaml
- name: Run with user environment
  shell:
    cmd: bash -l -c "mycommand"
  become: yes
  become_user: appuser
```

## See Also

- [command](command.md) - Execute commands without shell processing
- [script](script.md) - Run local scripts on remote hosts
- [template](template.md) - Generate scripts from templates
- [cron](cron.md) - Schedule shell commands
