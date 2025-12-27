# include_vars - Load Variables from Files

## Synopsis

The `include_vars` module loads variables from YAML or JSON files into the playbook scope during execution. Variables are loaded at IncludeVars precedence level, which is higher than most variable sources but lower than set_fact and extra_vars.

## Classification

**LocalLogic** - This module runs entirely on the control node and reads local files.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `file` | yes* | - | string | Path to a YAML/JSON file to load. |
| `dir` | yes* | - | string | Path to a directory containing variable files. |
| `name` | no | - | string | Scope all loaded variables under this key. |
| `depth` | no | 0 | integer | Depth of directory recursion (0 = no recursion). |

*Either `file` or `dir` must be provided, but not both.

## Return Values

The module returns the loaded variables directly in the `data` field.

| Key | Type | Description |
|-----|------|-------------|
| `<variable_name>` | any | Each variable loaded from the file(s) |

## Examples

### Load variables from a single file

```yaml
- name: Load application config
  include_vars:
    file: vars/app_config.yml
```

### Load from absolute path

```yaml
- name: Load secrets
  include_vars:
    file: /etc/myapp/secrets.yml
```

### Load variables from a directory

```yaml
- name: Load all config files
  include_vars:
    dir: vars/
```

### Scope variables under a key

```yaml
- name: Load database config under 'db' key
  include_vars:
    file: vars/database.yml
    name: db

- name: Use scoped variable
  debug:
    msg: "Database host: {{ db.host }}"
```

### Load environment-specific variables

```yaml
- name: Load environment config
  include_vars:
    file: "vars/{{ environment }}.yml"
```

### Load with conditional path

```yaml
- name: Load OS-specific variables
  include_vars:
    file: "vars/{{ ansible_os_family }}.yml"
```

## Example Variable Files

### YAML format (app_config.yml)

```yaml
app_name: "myapp"
app_version: "2.1.0"
app_settings:
  debug: false
  log_level: "info"
  max_connections: 100
```

### JSON format (database.json)

```json
{
  "host": "localhost",
  "port": 5432,
  "name": "mydb",
  "pool_size": 10
}
```

## Directory Loading

When using the `dir` parameter:

- Only files with `.yml`, `.yaml`, or `.json` extensions are loaded
- Files are processed in alphabetical order
- Variables from later files override earlier ones
- Non-variable files (like `.md` or `.txt`) are ignored

### Example directory structure

```
vars/
  01-base.yml
  02-network.yml
  03-database.yml
```

## Notes

- Variables loaded with `include_vars` have higher precedence than group_vars/host_vars
- Variables loaded later in the playbook override earlier ones
- The `name` parameter is useful to avoid variable name conflicts
- Both YAML and JSON files are supported
- Invalid YAML/JSON files will cause the task to fail
- In check mode, variables are still loaded (subsequent tasks may depend on them)
- Paths can be relative to the playbook or absolute

## Variable Precedence

Variables from `include_vars` have this precedence:
- Higher than: inventory variables, group_vars, host_vars
- Lower than: set_fact, register, extra_vars (-e)

## See Also

- [set_fact](set_fact.md) - Set variables dynamically
- [debug](debug.md) - Print variable values
