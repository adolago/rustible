# fail - Fail with Custom Message

## Synopsis

The `fail` module intentionally fails a playbook with a custom error message. It is useful for enforcing preconditions or providing clear error messages when conditions are not met.

## Classification

**LocalLogic** - This module runs entirely on the control node.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `msg` | no | "Failed as requested from task" | string | Custom failure message. |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `msg` | string | The failure message |
| `failed` | boolean | Always true |

## Examples

### Fail with custom message

```yaml
- name: Fail if not on supported OS
  fail:
    msg: "This playbook only supports Debian-based systems"
  when: ansible_os_family != 'Debian'
```

### Fail with default message

```yaml
- name: Unconditional failure
  fail:
```

### Fail with dynamic message

```yaml
- name: Fail with detailed message
  fail:
    msg: "Cannot proceed: {{ error_reason }}"
  when: error_condition
```

### Validate required variables

```yaml
- name: Check required variables
  fail:
    msg: "The variable '{{ item }}' is not defined"
  when: vars[item] is not defined
  loop:
    - app_name
    - app_version
    - deploy_environment
```

### Fail based on task result

```yaml
- name: Check service status
  command: systemctl is-active critical-service
  register: service_check
  ignore_errors: yes

- name: Fail if service is not running
  fail:
    msg: "Critical service is not running. Current status: {{ service_check.stdout }}"
  when: service_check.rc != 0
```

### Fail with assertion-like behavior

```yaml
- name: Validate configuration
  fail:
    msg: "Invalid port number: {{ app_port }}. Must be between 1024 and 65535."
  when: app_port < 1024 or app_port > 65535
```

## Notes

- The `fail` module always results in a failed task
- Use `when` conditions to make failures conditional
- For validating multiple conditions, consider using `assert` instead
- The module is useful for creating custom validation logic
- In check mode, the module behaves the same as in regular execution

## Comparison with Assert

| Use Case | Module |
|----------|--------|
| Validate multiple conditions | assert |
| Custom failure logic | fail |
| Simple condition check | either |
| Clear error messaging | fail |

## See Also

- [assert](assert.md) - Assert conditions are true
- [debug](debug.md) - Print messages without failing
