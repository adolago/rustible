# Quick Start: Debug Module

## What is the Debug Module?

The debug module is a diagnostic tool for Rustible playbooks. Use it to print messages and inspect variable values during playbook execution.

## Basic Usage

### Print a Message

```yaml
- name: Say hello
  debug:
    msg: "Hello, World!"
```

### Print a Variable

```yaml
- name: Show app version
  debug:
    var: app_version
```

## Common Patterns

### 1. Debugging Task Results

```yaml
- name: Run a command
  command: whoami
  register: result

- name: See what the command returned
  debug:
    var: result
```

### 2. Inspecting Configuration

```yaml
- name: Show database config
  debug:
    var: database_config
```

### 3. Nested Variables

```yaml
- name: Show hostname from facts
  debug:
    var: ansible_facts.hostname

- name: Show database host
  debug:
    var: app_config.database.host
```

### 4. Conditional Debugging

```yaml
- name: Show error message when task fails
  debug:
    msg: "Error occurred: {{ error_message }}"
  when: task_failed
```

### 5. Loop Debugging

```yaml
- name: Process each item
  debug:
    msg: "Processing: {{ item }}"
  loop:
    - item1
    - item2
    - item3
```

### 6. Verbose Debugging

```yaml
- name: Detailed information
  debug:
    msg: "This only shows with -vv"
    verbosity: 2
```

Run with: `rustible run playbook.yml -vv`

## Key Features

- **No SSH Required**: Runs locally on the control node
- **Never Fails**: Won't stop playbook execution
- **No Changes**: Always reports `changed: false`
- **Pretty Printing**: Automatically formats complex objects
- **Safe**: Can be used in production playbooks

## Parameters

| Parameter | Required | Type | Description |
|-----------|----------|------|-------------|
| `msg` | No* | string | Message to print |
| `var` | No* | string | Variable name to print |
| `verbosity` | No | integer | Minimum verbosity level (default: 0) |

*Either `msg` or `var` must be provided, but not both.

## Tips

1. **Use in Development**: Add debug tasks while developing playbooks, remove or comment out in production
2. **Register Results**: Always debug registered variables to see what data is available
3. **Check Undefined**: Debug will show "VARIABLE IS NOT DEFINED!" for missing variables
4. **Verbosity Levels**: Use verbosity to hide detailed debug info in normal runs
5. **No Performance Impact**: Debug is extremely fast, safe to use frequently

## Example Playbook

```yaml
---
- name: Debug Examples
  hosts: localhost
  connection: local

  vars:
    app_name: "MyApp"
    app_version: "1.0.0"

  tasks:
    - name: Print welcome message
      debug:
        msg: "Starting deployment of {{ app_name }}"

    - name: Show version
      debug:
        var: app_version

    - name: Run command
      command: date
      register: current_date

    - name: Show command result
      debug:
        var: current_date.stdout

    - name: Completion message
      debug:
        msg: "Deployment complete!"
```

## What's Different from Ansible?

The Rustible debug module is compatible with Ansible's debug module. Key features:

✓ Same parameters (msg, var, verbosity)
✓ Same behavior in check mode
✓ Same output format
✓ Nested variable access
✓ Pretty-printing of complex objects

The main difference is performance - Rustible's debug module is implemented in Rust and runs significantly faster than Ansible's Python implementation.
