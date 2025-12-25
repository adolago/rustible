# template - Template Files with Jinja2

## Synopsis

The `template` module templates a file to a remote location using Jinja2 templating. Variables and facts are available for substitution in the template.

## Classification

**NativeTransport** - This module uses native Rust operations for template rendering and SSH/SFTP for file transfer.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `src` | yes | - | string | Path to the template file on the local machine. |
| `dest` | yes | - | string | Remote absolute path where the file should be created. |
| `owner` | no | - | string | Name of the user that should own the file. |
| `group` | no | - | string | Name of the group that should own the file. |
| `mode` | no | - | string | Permissions of the file (e.g., "0644"). |
| `backup` | no | false | boolean | Create a backup file including the timestamp. |
| `force` | no | true | boolean | If false, only transfer if destination does not exist. |
| `validate` | no | - | string | Command to validate the file before use (use %s for file path). |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `dest` | string | Destination file path |
| `src` | string | Source template path |
| `checksum` | string | SHA1 checksum of the rendered file |
| `size` | integer | Size of the rendered file in bytes |
| `owner` | string | Owner of the file |
| `group` | string | Group of the file |
| `mode` | string | Permissions of the file |
| `backup_file` | string | Path to backup file (if backup was created) |

## Examples

### Template a configuration file

```yaml
- name: Template nginx configuration
  template:
    src: templates/nginx.conf.j2
    dest: /etc/nginx/nginx.conf
    owner: root
    group: root
    mode: "0644"
```

### Template with validation

```yaml
- name: Template Apache config with validation
  template:
    src: templates/httpd.conf.j2
    dest: /etc/httpd/conf/httpd.conf
    validate: httpd -t -f %s
```

### Template with backup

```yaml
- name: Update configuration with backup
  template:
    src: templates/app.conf.j2
    dest: /etc/myapp/app.conf
    backup: yes
```

### Example Template File (nginx.conf.j2)

```jinja2
# Managed by Rustible
worker_processes {{ worker_processes | default(4) }};

events {
    worker_connections {{ worker_connections | default(1024) }};
}

http {
    server {
        listen {{ http_port | default(80) }};
        server_name {{ server_name }};

        location / {
            root {{ document_root }};
        }

        {% if enable_ssl %}
        listen {{ https_port | default(443) }} ssl;
        ssl_certificate {{ ssl_cert_path }};
        ssl_certificate_key {{ ssl_key_path }};
        {% endif %}
    }
}
```

## Template Syntax

Rustible uses Jinja2-compatible syntax for templates:

| Syntax | Description |
|--------|-------------|
| `{{ variable }}` | Output a variable value |
| `{% if condition %}...{% endif %}` | Conditional blocks |
| `{% for item in list %}...{% endfor %}` | Loop over items |
| `{{ value \| filter }}` | Apply a filter to a value |
| `{# comment #}` | Template comments (not rendered) |

### Common Filters

| Filter | Description |
|--------|-------------|
| `default(value)` | Provide a default if variable is undefined |
| `upper` | Convert to uppercase |
| `lower` | Convert to lowercase |
| `trim` | Remove leading/trailing whitespace |
| `join(sep)` | Join list elements with separator |

## Notes

- Templates are rendered on the control node before being transferred
- All variables and facts are available in templates
- The module is idempotent; it will not update files if rendered content is identical
- Template files typically use the `.j2` extension by convention
- Invalid template syntax will cause the task to fail

## See Also

- [copy](copy.md) - Copy files without templating
- [lineinfile](lineinfile.md) - Manage specific lines in files
