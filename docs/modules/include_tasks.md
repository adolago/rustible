# include_tasks - Dynamically Include Tasks

## Synopsis

The `include_tasks` module dynamically includes a task file during playbook execution. This allows for modular playbook organization and conditional task inclusion.

## Classification

**LocalLogic** - This module runs entirely on the control node.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `file` | yes | - | string | Path to the task file to include. |
| `apply` | no | - | object | Tags or other task attributes to apply to included tasks. |

## Return Values

The module does not return specific values. Included tasks execute as part of the play.

## Examples

### Include a task file

```yaml
- name: Include installation tasks
  include_tasks: tasks/install.yml
```

### Include with variable path

```yaml
- name: Include OS-specific tasks
  include_tasks: "tasks/{{ ansible_os_family | lower }}.yml"
```

### Include conditionally

```yaml
- name: Include setup tasks if needed
  include_tasks: tasks/setup.yml
  when: needs_setup | bool
```

### Include with loop

```yaml
- name: Configure each application
  include_tasks: tasks/configure_app.yml
  loop: "{{ applications }}"
  loop_control:
    loop_var: app_config
```

### Include with tags

```yaml
- name: Include deployment tasks
  include_tasks:
    file: tasks/deploy.yml
    apply:
      tags:
        - deploy
        - update
```

### Include with variables

```yaml
- name: Include database setup
  include_tasks: tasks/setup_database.yml
  vars:
    db_name: myapp
    db_user: appuser
```

## Example Task File (tasks/install.yml)

```yaml
---
- name: Install required packages
  package:
    name: "{{ item }}"
    state: present
  loop: "{{ required_packages }}"

- name: Create application directory
  file:
    path: /opt/myapp
    state: directory
    mode: '0755'

- name: Copy application files
  copy:
    src: files/myapp/
    dest: /opt/myapp/
```

## Use Cases

### Modular Playbook Organization

```yaml
# main.yml
- hosts: all
  tasks:
    - include_tasks: tasks/prerequisites.yml
    - include_tasks: tasks/install.yml
    - include_tasks: tasks/configure.yml
    - include_tasks: tasks/validate.yml
```

### Environment-Specific Tasks

```yaml
- name: Include environment config
  include_tasks: "tasks/{{ environment }}/setup.yml"
```

### Conditional Feature Installation

```yaml
- name: Install monitoring
  include_tasks: tasks/monitoring.yml
  when: enable_monitoring | bool

- name: Install logging
  include_tasks: tasks/logging.yml
  when: enable_logging | bool
```

## Notes

- Included task files should be valid YAML task lists
- Variables set in the including play are available to included tasks
- Loop variables are available using `loop_var` from `loop_control`
- The `apply` parameter allows setting tags and other attributes on all included tasks
- Files are searched relative to the playbook directory
- In check mode, included tasks are processed but may show different results

## Comparison with import_tasks

| Feature | include_tasks | import_tasks |
|---------|---------------|--------------|
| When processed | Runtime | Parse time |
| Conditional include | Yes | Limited |
| Loop support | Yes | No |
| Variable paths | Yes | Yes |
| Tag inheritance | Via apply | Automatic |

## See Also

- [include_vars](include_vars.md) - Include variable files
- [set_fact](set_fact.md) - Set variables for included tasks
