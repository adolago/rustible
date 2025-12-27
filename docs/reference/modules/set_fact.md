# set_fact - Set Host Variables

## Synopsis

The `set_fact` module sets host variables dynamically during playbook execution. Unlike gathered facts, these are user-defined variables that persist for the duration of the play.

## Classification

**LocalLogic** - This module runs entirely on the control node and does not require a connection to remote hosts.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `key=value` | yes | - | any | One or more key=value pairs to set as facts. |
| `cacheable` | no | false | boolean | Cache the facts for subsequent plays. |

## Return Values

The module returns the facts that were set in the `data` field.

| Key | Type | Description |
|-----|------|-------------|
| `<fact_name>` | any | Each fact that was set |
| `cacheable` | boolean | Whether facts are cacheable |

## Examples

### Set a simple fact

```yaml
- name: Set application version
  set_fact:
    app_version: "2.1.0"
```

### Set multiple facts

```yaml
- name: Set application configuration
  set_fact:
    app_name: "myapp"
    app_port: 8080
    app_debug: true
```

### Set facts from task output

```yaml
- name: Get current date
  command: date +%Y-%m-%d
  register: date_result

- name: Set date as fact
  set_fact:
    deployment_date: "{{ date_result.stdout }}"
```

### Set complex data structures

```yaml
- name: Set configuration dictionary
  set_fact:
    database_config:
      host: "localhost"
      port: 5432
      name: "mydb"
      credentials:
        username: "admin"
        password: "{{ vault_db_password }}"
```

### Set list facts

```yaml
- name: Set server list
  set_fact:
    backend_servers:
      - "server1.example.com"
      - "server2.example.com"
      - "server3.example.com"
```

### Combine existing facts

```yaml
- name: Build full URL
  set_fact:
    app_url: "https://{{ app_host }}:{{ app_port }}/{{ app_path }}"
```

### Conditional fact setting

```yaml
- name: Set environment-specific values
  set_fact:
    log_level: "{{ 'debug' if environment == 'dev' else 'info' }}"
```

### Set cacheable facts

```yaml
- name: Set cacheable fact
  set_fact:
    expensive_calculation_result: "{{ result }}"
    cacheable: yes
```

### Compute values from other facts

```yaml
- name: Calculate derived values
  set_fact:
    total_memory_mb: "{{ ansible_memtotal_mb }}"
    memory_for_app: "{{ (ansible_memtotal_mb * 0.8) | int }}"
```

## Notes

- Facts set with `set_fact` persist for the duration of the current play
- The `cacheable` option allows facts to persist across plays using fact caching
- Facts set this way have higher precedence than most other variable sources
- In check mode, `set_fact` behaves the same as in regular execution
- At least one key=value pair must be provided (excluding `cacheable`)
- The module always reports `ok` status (never `changed`)

## Precedence

Variables set with `set_fact` have the following precedence:
- Higher than: inventory variables, group_vars, host_vars, include_vars
- Lower than: extra_vars passed via command line (-e)

## Real-World Use Cases

### Dynamic Configuration

```yaml
- name: Set environment-specific configuration
  set_fact:
    app_config:
      debug: "{{ environment == 'development' }}"
      log_level: "{{ 'DEBUG' if environment == 'development' else 'INFO' }}"
      db_host: "{{ 'localhost' if environment == 'development' else db_cluster_host }}"
      cache_ttl: "{{ 60 if environment == 'development' else 3600 }}"
```

### Computed Values

```yaml
- name: Calculate resource allocations
  set_fact:
    jvm_heap_size: "{{ (ansible_memtotal_mb * 0.75) | int }}m"
    worker_processes: "{{ ansible_processor_vcpus }}"
    max_connections: "{{ ansible_processor_vcpus * 1000 }}"
```

### Building Connection Strings

```yaml
- name: Build database connection string
  set_fact:
    database_url: "postgresql://{{ db_user }}:{{ db_password }}@{{ db_host }}:{{ db_port }}/{{ db_name }}"
    redis_url: "redis://{{ redis_host }}:{{ redis_port }}/{{ redis_db }}"
```

### Aggregating Results

```yaml
- name: Check services
  command: systemctl is-active {{ item }}
  register: service_checks
  loop:
    - nginx
    - postgresql
    - redis

- name: Set service status
  set_fact:
    failed_services: "{{ service_checks.results | selectattr('rc', 'ne', 0) | map(attribute='item') | list }}"
    all_services_healthy: "{{ service_checks.results | selectattr('rc', 'ne', 0) | list | length == 0 }}"
```

## Troubleshooting

### Variable not available in later tasks

Variables set with `set_fact` are host-scoped. For cross-host variables:

```yaml
# Set on all hosts
- set_fact:
    my_var: "value"

# Access from another host
- debug:
    msg: "{{ hostvars['other_host']['my_var'] }}"
```

### Variable not persisting across plays

Use `cacheable: yes` with fact caching enabled:

```yaml
- set_fact:
    persistent_value: "{{ result }}"
    cacheable: yes
```

### Overwritten by include_vars

Check variable precedence. Use extra_vars (`-e`) for highest priority:

```bash
rustible-playbook playbook.yml -e "my_var=override_value"
```

### Complex data structure not working

Ensure proper YAML syntax:

```yaml
# CORRECT - nested dictionary
- set_fact:
    config:
      section1:
        key1: value1
        key2: value2

# CORRECT - list of dictionaries
- set_fact:
    servers:
      - name: server1
        ip: 10.0.0.1
      - name: server2
        ip: 10.0.0.2
```

### Variable evaluation issues

For complex expressions, break them down:

```yaml
- name: Calculate intermediate value
  set_fact:
    total_memory: "{{ ansible_memtotal_mb }}"

- name: Use intermediate value
  set_fact:
    allocated_memory: "{{ (total_memory | int * 0.8) | int }}"
```

### Type coercion problems

Explicitly convert types:

```yaml
- set_fact:
    port_number: "{{ input_port | int }}"
    enabled: "{{ input_flag | bool }}"
    server_list: "{{ input_string | split(',') }}"
```

## See Also

- [debug](debug.md) - Print variable values
- [include_vars](include_vars.md) - Load variables from files
- [assert](assert.md) - Assert conditions
- [stat](stat.md) - Get file info to set as facts
- [command](command.md) - Run commands and set results as facts
