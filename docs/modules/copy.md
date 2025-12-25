# copy - Copy Files to Remote Locations

## Synopsis

The `copy` module copies files from the local machine to remote locations. It can also copy content directly to a remote file.

## Classification

**NativeTransport** - This module uses native Rust SSH/SFTP operations for file transfer.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `src` | yes* | - | string | Local path to file to copy. Mutually exclusive with `content`. |
| `content` | yes* | - | string | Content to write directly to the destination file. |
| `dest` | yes | - | string | Remote absolute path where the file should be copied. |
| `owner` | no | - | string | Name of the user that should own the file. |
| `group` | no | - | string | Name of the group that should own the file. |
| `mode` | no | - | string | Permissions of the file (e.g., "0644" or "u=rw,g=r,o=r"). |
| `backup` | no | false | boolean | Create a backup file including the timestamp. |
| `force` | no | true | boolean | If false, only transfer if destination does not exist. |
| `validate` | no | - | string | Command to validate the file before use (use %s for file path). |

*Either `src` or `content` must be provided.

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `dest` | string | Destination file path |
| `src` | string | Source file path (if used) |
| `checksum` | string | SHA1 checksum of the file |
| `size` | integer | Size of the file in bytes |
| `owner` | string | Owner of the file |
| `group` | string | Group of the file |
| `mode` | string | Permissions of the file |
| `backup_file` | string | Path to backup file (if backup was created) |

## Examples

### Copy a file with specific permissions

```yaml
- name: Copy configuration file
  copy:
    src: files/app.conf
    dest: /etc/myapp/app.conf
    owner: root
    group: root
    mode: "0644"
```

### Copy content directly to a file

```yaml
- name: Create a configuration file from content
  copy:
    content: |
      [myapp]
      setting1 = value1
      setting2 = value2
    dest: /etc/myapp/settings.ini
    mode: "0640"
```

### Copy with backup

```yaml
- name: Update configuration with backup
  copy:
    src: files/nginx.conf
    dest: /etc/nginx/nginx.conf
    backup: yes
```

### Copy only if destination does not exist

```yaml
- name: Copy default config if not present
  copy:
    src: files/default.conf
    dest: /etc/myapp/config.conf
    force: no
```

### Validate configuration before applying

```yaml
- name: Copy nginx config with validation
  copy:
    src: files/nginx.conf
    dest: /etc/nginx/nginx.conf
    validate: nginx -t -c %s
```

## Notes

- The `copy` module is idempotent; it will not copy files if they are identical
- Checksums are used to determine if a file has changed
- When using `content`, the file is created even if empty
- Symbolic mode notation (like "u=rw,g=r,o=r") is supported
- The module creates parent directories if they do not exist

## See Also

- [template](template.md) - Template files with variable substitution
- [file](file.md) - Manage file and directory properties
