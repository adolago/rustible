# wait_for - Wait for a Condition

## Synopsis

The `wait_for` module waits for a condition to be met before continuing. It can wait for ports to become available, files to exist, or processes to start/stop.

## Classification

**RemoteCommand** - This module executes checks on remote hosts via SSH.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `host` | no | 127.0.0.1 | string | Host to check for port availability. |
| `port` | no | - | integer | Port number to poll. |
| `path` | no | - | string | File path to check for existence. |
| `search_regex` | no | - | string | Regex to match in file or socket response. |
| `state` | no | started | string | State to wait for: started, stopped, present, absent, drained. |
| `delay` | no | 0 | integer | Seconds to wait before starting checks. |
| `timeout` | no | 300 | integer | Maximum seconds to wait. |
| `sleep` | no | 1 | integer | Seconds between retries. |
| `connect_timeout` | no | 5 | integer | Seconds to wait for connection. |
| `msg` | no | - | string | Custom message for timeout error. |
| `active_connection_states` | no | - | list | Connection states to consider active. |
| `exclude_hosts` | no | - | list | Hosts to exclude from drain check. |

## State Values

| State | Description |
|-------|-------------|
| `started` | Port is open or file exists |
| `stopped` | Port is closed or file does not exist |
| `present` | File exists |
| `absent` | File does not exist |
| `drained` | No active connections on port |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `elapsed` | integer | Seconds waited |
| `match_groups` | list | Regex match groups (if search_regex used) |
| `match_groupdict` | object | Named regex groups (if search_regex used) |

## Examples

### Wait for port to be available

```yaml
- name: Wait for application to start
  wait_for:
    port: 8080
    state: started
```

### Wait for remote host port

```yaml
- name: Wait for database to be ready
  wait_for:
    host: db.example.com
    port: 5432
    timeout: 60
```

### Wait for file to exist

```yaml
- name: Wait for lock file to be created
  wait_for:
    path: /var/run/myapp.lock
    state: present
```

### Wait for file to be removed

```yaml
- name: Wait for installation to complete
  wait_for:
    path: /tmp/installing.lock
    state: absent
    timeout: 600
```

### Wait for file with specific content

```yaml
- name: Wait for status file to contain 'ready'
  wait_for:
    path: /var/run/myapp/status
    search_regex: "ready"
```

### Wait for port to close

```yaml
- name: Wait for old process to stop
  wait_for:
    port: 8080
    state: stopped
```

### Wait for connections to drain

```yaml
- name: Wait for connections to drain before shutdown
  wait_for:
    port: 80
    state: drained
    exclude_hosts:
      - 127.0.0.1
```

### Wait with delay before checking

```yaml
- name: Wait after service restart
  wait_for:
    port: 8080
    delay: 10
    timeout: 60
```

### Custom timeout message

```yaml
- name: Wait for critical service
  wait_for:
    port: 3306
    timeout: 120
    msg: "MySQL did not start within 2 minutes"
```

### Wait for socket response

```yaml
- name: Wait for HTTP response
  wait_for:
    port: 80
    search_regex: "HTTP/1.1 200"
```

## Notes

- The module runs on the target host, not the control node
- Port checks use TCP connections
- The `drained` state waits for existing connections to close
- Use `delay` to give services time to fully start
- The `timeout` value should account for worst-case startup time
- In check mode, the wait is not performed

## Common Use Cases

| Scenario | Configuration |
|----------|---------------|
| Service startup | `port: X, state: started` |
| Service shutdown | `port: X, state: stopped` |
| File creation | `path: X, state: present` |
| File deletion | `path: X, state: absent` |
| Graceful shutdown | `port: X, state: drained` |

## See Also

- [pause](pause.md) - Simple time-based pause
- [service](service.md) - Manage services
