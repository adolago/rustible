# lineinfile - Manage Lines in Text Files

## Synopsis

The `lineinfile` module ensures a particular line is in a file, or replaces an existing line using a regex. It is primarily useful when you want to change a single line in a file.

## Classification

**NativeTransport** - This module uses native Rust operations for file manipulation and SSH/SFTP for transfer.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `path` | yes | - | string | Path to the file to modify. |
| `line` | yes* | - | string | The line to insert/replace. Required unless state=absent. |
| `regexp` | no | - | string | Regex to find the line to replace. |
| `state` | no | present | string | Desired state: present, absent. |
| `insertafter` | no | EOF | string | Insert after this regex or EOF/BOF. |
| `insertbefore` | no | - | string | Insert before this regex or BOF. |
| `create` | no | false | boolean | Create file if it does not exist. |
| `backup` | no | false | boolean | Create backup before modifying. |
| `backrefs` | no | false | boolean | Use backreferences in line from regexp groups. |
| `firstmatch` | no | false | boolean | Only replace the first match. |
| `owner` | no | - | string | Owner of the file. |
| `group` | no | - | string | Group of the file. |
| `mode` | no | - | string | Permissions of the file. |

## State Values

| State | Description |
|-------|-------------|
| `present` | Ensure the line is in the file |
| `absent` | Ensure lines matching regexp are removed |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `changed` | boolean | Whether the file was modified |
| `msg` | string | Status message |
| `backup` | string | Path to backup file (if created) |

## Examples

### Add a line to a file

```yaml
- name: Add line to hosts file
  lineinfile:
    path: /etc/hosts
    line: "192.168.1.100 myserver.local"
```

### Replace a line using regex

```yaml
- name: Update SSH port
  lineinfile:
    path: /etc/ssh/sshd_config
    regexp: '^#?Port\s+'
    line: 'Port 2222'
```

### Remove lines matching a pattern

```yaml
- name: Remove old entries
  lineinfile:
    path: /etc/hosts
    regexp: '.*oldserver.*'
    state: absent
```

### Insert after a specific line

```yaml
- name: Add after specific line
  lineinfile:
    path: /etc/fstab
    insertafter: '^# End of standard entries'
    line: '/dev/sdb1 /data ext4 defaults 0 2'
```

### Insert before a specific line

```yaml
- name: Add before specific line
  lineinfile:
    path: /etc/nginx/nginx.conf
    insertbefore: '^http {'
    line: '# Custom configuration'
```

### Create file if not exists

```yaml
- name: Ensure line in new file
  lineinfile:
    path: /etc/myapp/config.txt
    line: "setting=value"
    create: yes
    mode: '0644'
```

### Use backreferences

```yaml
- name: Update version number
  lineinfile:
    path: /etc/myapp/version.conf
    regexp: '^version=(.*)$'
    line: 'version=\1-updated'
    backrefs: yes
```

### Create backup before modifying

```yaml
- name: Modify with backup
  lineinfile:
    path: /etc/important.conf
    regexp: '^old_setting='
    line: 'old_setting=new_value'
    backup: yes
```

## Notes

- If `regexp` is provided and matches, the line is replaced
- If `regexp` is provided but does not match, the line is added
- If `regexp` is not provided, the line is added only if not already present
- The `backrefs` option requires `regexp` and uses `\1`, `\2`, etc. for groups
- Use `blockinfile` for managing multiple lines as a block

## See Also

- [blockinfile](blockinfile.md) - Manage blocks of text in files
- [copy](copy.md) - Copy files
- [template](template.md) - Template files
