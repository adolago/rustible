# pause - Pause Playbook Execution

## Synopsis

The `pause` module pauses playbook execution for a specified period of time or until user input. It is useful for waiting for external processes, user confirmation, or introducing delays.

## Classification

**LocalLogic** - This module runs entirely on the control node.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `seconds` | no | - | integer | Number of seconds to pause. |
| `minutes` | no | - | integer | Number of minutes to pause. |
| `prompt` | no | - | string | Prompt message for user input pause. |
| `echo` | no | true | boolean | Echo user input (set to false for passwords). |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `delta` | string | Time elapsed during pause |
| `start` | string | Start timestamp |
| `stop` | string | End timestamp |
| `user_input` | string | User input (if prompt was used) |

## Examples

### Pause for a specific duration

```yaml
- name: Wait for service to initialize
  pause:
    seconds: 30
```

### Pause for minutes

```yaml
- name: Wait for long-running process
  pause:
    minutes: 5
```

### Pause for user confirmation

```yaml
- name: Confirm before proceeding
  pause:
    prompt: "Press Enter to continue or Ctrl+C to abort"
```

### Pause with input prompt

```yaml
- name: Get user input
  pause:
    prompt: "Enter the deployment target environment"
  register: env_input

- name: Use the input
  debug:
    msg: "Deploying to {{ env_input.user_input }}"
```

### Pause for sensitive input

```yaml
- name: Get password without echoing
  pause:
    prompt: "Enter database password"
    echo: no
  register: password_input
```

### Conditional pause

```yaml
- name: Wait between batches
  pause:
    seconds: 60
  when: batch_index > 0
```

### Pause with timeout notice

```yaml
- name: Pause with informative message
  debug:
    msg: "Waiting 60 seconds for DNS propagation..."

- name: Wait for DNS
  pause:
    seconds: 60
```

## Notes

- When no parameters are given, the pause waits for user input (Enter key)
- The `prompt` parameter is only effective in interactive sessions
- In non-interactive environments, prompts will fail or timeout
- Use `seconds` or `minutes`, not both
- The pause duration does not count against task timeout
- In check mode, the pause is not executed

## Use Cases

| Scenario | Recommended Usage |
|----------|-------------------|
| Service startup | `seconds: 30` |
| DNS propagation | `minutes: 5` |
| User confirmation | `prompt: "message"` |
| Rate limiting | `seconds: 1` in loops |
| Manual verification | `prompt: "Verify and press Enter"` |

## See Also

- [wait_for](wait_for.md) - Wait for a condition
- [debug](debug.md) - Print status messages
