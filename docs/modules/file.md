# file - Manage File and Directory Properties

## Synopsis

The `file` module manages file and directory properties including state, permissions, ownership, and symbolic links. It can create, delete, and modify files and directories.

## Classification

**NativeTransport** - This module uses native Rust SSH/SFTP operations.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `path` | yes | - | string | Path to the file or directory to manage. |
| `state` | no | file | string | Desired state: file, directory, link, hard, touch, absent. |
| `owner` | no | - | string | Name of the user that should own the file/directory. |
| `group` | no | - | string | Name of the group that should own the file/directory. |
| `mode` | no | - | string | Permissions of the file/directory. |
| `src` | no | - | string | Path to the file to link to (for state=link or state=hard). |
| `force` | no | false | boolean | Force creation of symlinks even if source does not exist. |
| `recurse` | no | false | boolean | Apply owner, group, mode recursively to directories. |

## State Values

| State | Description |
|-------|-------------|
| `file` | Ensure file exists and has specified properties |
| `directory` | Ensure directory exists and has specified properties |
| `link` | Create a symbolic link |
| `hard` | Create a hard link |
| `touch` | Create empty file if not exists, update mtime if exists |
| `absent` | Remove the file or directory |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `path` | string | Path that was managed |
| `state` | string | State of the file/directory |
| `mode` | string | Permissions of the file/directory |
| `owner` | string | Owner of the file/directory |
| `group` | string | Group of the file/directory |
| `size` | integer | Size of the file in bytes |

## Examples

### Create a directory with specific permissions

```yaml
- name: Ensure app directory exists
  file:
    path: /var/lib/myapp
    state: directory
    owner: appuser
    group: appgroup
    mode: "0755"
```

### Create a symbolic link

```yaml
- name: Create a symlink
  file:
    path: /usr/local/bin/myapp
    src: /opt/myapp/bin/myapp
    state: link
```

### Remove a file

```yaml
- name: Remove temporary file
  file:
    path: /tmp/myapp.tmp
    state: absent
```

### Set file permissions

```yaml
- name: Set permissions on a file
  file:
    path: /etc/myapp/secret.conf
    mode: "0600"
    owner: root
    group: root
```

### Create an empty file (touch)

```yaml
- name: Touch a file to update timestamp
  file:
    path: /var/log/myapp/last_run
    state: touch
```

### Recursively set permissions on a directory

```yaml
- name: Set permissions recursively
  file:
    path: /var/www/html
    owner: www-data
    group: www-data
    mode: "0755"
    recurse: yes
```

## Notes

- When `state=file`, the file must already exist; it will not create a new file
- Use `state=touch` to create an empty file if it does not exist
- The `recurse` option only works with `state=directory`
- Hard links cannot span filesystems
- Symbolic links can point to non-existent targets when `force=yes`

## See Also

- [copy](copy.md) - Copy files to remote locations
- [template](template.md) - Template files with variable substitution
- [stat](stat.md) - Retrieve file information
