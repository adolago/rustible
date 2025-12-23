# Debug Module

The `debug` module prints debug messages or variable values to the console. It's primarily used for debugging playbooks during development.

## Module Classification

**LocalLogic** - This module runs entirely on the control node and does not require an SSH connection to target hosts.

## Parameters

### Required Parameters

Either `msg` or `var` must be provided (but not both).

### Optional Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `msg` | string | - | A message to print. Can include Jinja2 template variables. |
| `var` | string | - | The name of a variable to print. Supports nested paths like `ansible_facts.hostname`. |
| `verbosity` | integer | 0 | Only show this message when the verbosity level is at or above this value. |

## Return Values

The debug module returns:

- `changed`: Always `false` (debug never changes anything)
- `msg`: The formatted message that was printed
- `data`: A dictionary containing the variable name and value (when using `var`)

## Examples

### Basic Usage

Print a simple message:

```yaml
- name: Print hello world
  debug:
    msg: "Hello, World!"
```

Print a variable value:

```yaml
- name: Show app version
  debug:
    var: app_version
```

### Using Template Variables

You can use Jinja2 template syntax in messages:

```yaml
- name: Print deployment info
  debug:
    msg: "Deploying {{ app_name }} version {{ app_version }} to {{ environment }}"
```

### Printing Complex Objects

The debug module can pretty-print complex data structures:

```yaml
- name: Show entire configuration
  debug:
    var: app_config

- name: Show nested value
  debug:
    var: app_config.database.host
```

### Using Verbosity Levels

Control when messages are shown using verbosity:

```yaml
- name: Detailed debug info
  debug:
    msg: "This only shows with -vv or higher"
    verbosity: 2
```

Run with: `rustible run playbook.yml -vv`

### Debugging Undefined Variables

The debug module handles undefined variables gracefully:

```yaml
- name: Check if variable exists
  debug:
    var: possibly_undefined_var
```

This will print `VARIABLE IS NOT DEFINED!` if the variable doesn't exist.

## Notes

1. The debug module never causes a task to fail, even if a variable is undefined.
2. Unlike most modules, debug runs on the control node (localhost) and doesn't require a connection to target hosts.
3. The debug module behaves identically in check mode and normal mode.
4. The `changed` status is always `false` since debug never modifies anything.

## Check Mode Support

Fully supported. The debug module behaves the same in check mode and normal mode.

## Diff Mode Support

Not applicable. The debug module never generates diffs.

## Ansible Compatibility

This module provides the same functionality as Ansible's `debug` module with support for:

- `msg` parameter for custom messages
- `var` parameter for printing variables
- `verbosity` parameter for conditional output
- Nested variable access (e.g., `ansible_facts.hostname`)
- Pretty-printing of complex data structures

## Performance Notes

The debug module is extremely fast since it runs locally and only performs string formatting. It's classified as `LocalLogic` which means:

- No SSH connection required
- No remote execution
- Executes in microseconds
- Safe to use in hot loops

## Common Use Cases

### 1. Development and Troubleshooting

```yaml
- name: Debug task results
  command: whoami
  register: result

- name: Show what command returned
  debug:
    var: result
```

### 2. Conditional Debugging

```yaml
- name: Show error details when something fails
  debug:
    msg: "Error: {{ error_message }}"
  when: task_failed
```

### 3. Configuration Validation

```yaml
- name: Verify configuration before proceeding
  debug:
    msg: "Will deploy to: {{ inventory_hostname }} with config: {{ app_config }}"
```

### 4. Loop Debugging

```yaml
- name: Process items
  debug:
    msg: "Processing item: {{ item }}"
  loop:
    - item1
    - item2
    - item3
```

## See Also

- [set_fact](set_fact.md) - Set variables
- [assert](assert.md) - Validate conditions
- [fail](fail.md) - Fail with a message
