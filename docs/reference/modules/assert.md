# assert - Assert Conditions

## Synopsis

The `assert` module evaluates conditions and fails if they are not true. It is useful for validating preconditions before proceeding with a playbook.

## Classification

**LocalLogic** - This module runs entirely on the control node and does not require a connection to remote hosts.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `that` | yes | - | string/list | Condition(s) to evaluate. All must be true. |
| `msg` | no | - | string | Custom message to display on failure. |
| `success_msg` | no | - | string | Custom message to display on success. |
| `quiet` | no | false | boolean | Suppress output of evaluated conditions. |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `assertion` | list | List of all conditions that were evaluated |
| `evaluated_to` | list | Conditions that passed (in success case) |
| `failed_conditions` | list | Conditions that failed (in failure case) |

## Examples

### Assert a single condition

```yaml
- name: Ensure we are on the right OS
  assert:
    that: ansible_os_family == 'Debian'
```

### Assert multiple conditions

```yaml
- name: Validate deployment prerequisites
  assert:
    that:
      - ansible_memtotal_mb >= 4096
      - ansible_processor_vcpus >= 2
      - ansible_distribution == 'Ubuntu'
```

### Assert with custom failure message

```yaml
- name: Check disk space
  assert:
    that: ansible_mounts | selectattr('mount', 'equalto', '/') | map(attribute='size_available') | first > 10737418240
    msg: "Insufficient disk space on root partition. Need at least 10GB free."
```

### Assert with success message

```yaml
- name: Verify configuration
  assert:
    that: config_valid == true
    success_msg: "Configuration validated successfully"
    msg: "Configuration validation failed"
```

### Assert using variables

```yaml
- name: Ensure correct app version
  assert:
    that:
      - app_version is defined
      - app_version is version('2.0.0', '>=')
    msg: "Application version must be 2.0.0 or higher"
```

### Quiet assertions for cleaner output

```yaml
- name: Perform multiple checks quietly
  assert:
    that:
      - service_port > 0
      - service_port < 65536
      - database_host != ''
    quiet: yes
```

### Assert using registered variables

```yaml
- name: Check service status
  command: systemctl is-active nginx
  register: nginx_status
  ignore_errors: yes

- name: Assert nginx is running
  assert:
    that: nginx_status.rc == 0
    msg: "Nginx service is not running"
```

### Assert with logical operators

```yaml
- name: Validate environment
  assert:
    that:
      - (environment == 'prod') or (environment == 'staging')
      - api_key is defined and api_key != ''
    msg: "Invalid environment or missing API key"
```

### Assert file conditions

```yaml
- name: Check config file
  stat:
    path: /etc/myapp/config.yml
  register: config_file

- name: Assert config exists and is readable
  assert:
    that:
      - config_file.stat.exists
      - config_file.stat.readable
    msg: "Config file missing or not readable"
```

## Notes

- All conditions in the `that` parameter must evaluate to true
- Conditions are evaluated using Jinja2 expressions
- The module fails immediately when any condition is false
- Undefined variables in conditions will cause failure
- In check mode, assertions are still evaluated
- The `quiet` option only affects output, not behavior

## Common Condition Examples

| Condition | Description |
|-----------|-------------|
| `var is defined` | Variable exists |
| `var is not none` | Variable is not null |
| `var \| length > 0` | Variable is not empty |
| `var == 'value'` | Variable equals value |
| `var in ['a', 'b']` | Variable is in list |
| `var is version('1.0', '>=')` | Version comparison |
| `var \| int > 100` | Numeric comparison |

## Real-World Use Cases

### Pre-deployment Validation

```yaml
- name: Validate deployment prerequisites
  assert:
    that:
      - ansible_distribution == 'Ubuntu'
      - ansible_distribution_version is version('20.04', '>=')
      - ansible_memtotal_mb >= 4096
      - ansible_processor_vcpus >= 2
    fail_msg: "System does not meet deployment requirements"
    success_msg: "System meets all deployment requirements"
```

### Configuration Validation

```yaml
- name: Validate application configuration
  assert:
    that:
      - app_port is defined
      - app_port | int > 1024
      - app_port | int < 65535
      - database_host is defined
      - database_host != ''
      - api_key is defined
      - api_key | length >= 32
    fail_msg: "Invalid application configuration"
```

### Security Compliance Check

```yaml
- name: Check file permissions compliance
  stat:
    path: "{{ item }}"
  register: perm_check
  loop:
    - /etc/shadow
    - /etc/passwd
    - /etc/sudoers

- name: Assert secure permissions
  assert:
    that:
      - item.stat.mode in ['0600', '0640', '0644', '0440']
      - item.stat.uid == 0
    fail_msg: "{{ item.item }} has insecure permissions: {{ item.stat.mode }}"
  loop: "{{ perm_check.results }}"
  when: item.stat.exists
```

### Service Health Check

```yaml
- name: Check service response
  uri:
    url: "http://localhost:{{ app_port }}/health"
  register: health_check

- name: Assert service is healthy
  assert:
    that:
      - health_check.status == 200
      - health_check.json.status == 'healthy'
      - health_check.json.database == 'connected'
    fail_msg: "Service health check failed: {{ health_check.json | default('no response') }}"
```

## Troubleshooting

### Assertion fails but condition looks correct

Debug the actual values:

```yaml
- debug:
    msg: |
      Variable value: {{ my_var }}
      Type: {{ my_var | type_debug }}
      Length: {{ my_var | length }}

- assert:
    that: my_var == 'expected'
```

### Undefined variable errors

Use `is defined` check:

```yaml
- assert:
    that:
      - my_var is defined
      - my_var is not none
      - my_var | length > 0
```

### Numeric comparisons failing

Ensure proper type conversion:

```yaml
# WRONG - string comparison
- assert:
    that: port > 1024

# CORRECT - explicit integer conversion
- assert:
    that: port | int > 1024
```

### Version comparison issues

Use the `version` test:

```yaml
- assert:
    that:
      - app_version is version('2.0.0', '>=')
      - python_version is version('3.8', '>=')
```

### Complex logic not working

Break down complex conditions:

```yaml
# WRONG - complex inline
- assert:
    that: (a and b) or (c and d) and not e

# CORRECT - separate conditions
- assert:
    that:
      - (a and b) or (c and d)
      - not e
```

### Quiet mode hiding useful info

When debugging, turn off quiet mode:

```yaml
- assert:
    that: condition
    quiet: no  # Show all evaluated conditions
```

### Multiple assertions masking failures

Run assertions separately for clearer error messages:

```yaml
- assert:
    that: memory_check
    fail_msg: "Insufficient memory"

- assert:
    that: cpu_check
    fail_msg: "Insufficient CPU"

- assert:
    that: disk_check
    fail_msg: "Insufficient disk space"
```

## See Also

- [debug](debug.md) - Print debug information
- [set_fact](set_fact.md) - Set variables
- [stat](stat.md) - Get file information for assertions
- [fail](fail.md) - Explicitly fail with a message
- [command](command.md) - Run commands to gather data for assertions
