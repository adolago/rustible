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

## Real-World Use Cases

### SSH Configuration

```yaml
- name: Disable root SSH login
  lineinfile:
    path: /etc/ssh/sshd_config
    regexp: '^#?PermitRootLogin'
    line: 'PermitRootLogin no'
  notify: Restart sshd

- name: Set SSH port
  lineinfile:
    path: /etc/ssh/sshd_config
    regexp: '^#?Port\s+'
    line: 'Port 2222'
  notify: Restart sshd
```

### Kernel Parameters

```yaml
- name: Add kernel module to load at boot
  lineinfile:
    path: /etc/modules-load.d/custom.conf
    line: "br_netfilter"
    create: yes
```

### Application Configuration

```yaml
- name: Set application database host
  lineinfile:
    path: /etc/myapp/database.conf
    regexp: '^db_host='
    line: 'db_host={{ db_server }}'
    create: yes
    backup: yes
```

### System Limits

```yaml
- name: Set file descriptor limits
  lineinfile:
    path: /etc/security/limits.conf
    regexp: '^\*\s+soft\s+nofile'
    line: '*                soft    nofile          65535'
    insertbefore: '^# End of file'
```

## Troubleshooting

### Line added multiple times

Use `regexp` to match existing lines:

```yaml
# WRONG - adds line each run if not exact match
- lineinfile:
    path: /etc/config
    line: "setting = value"

# CORRECT - replaces any line starting with 'setting'
- lineinfile:
    path: /etc/config
    regexp: '^setting\s*='
    line: "setting = value"
```

### Regex not matching

Test your regex:

```bash
grep -E '^#?Port\s+' /etc/ssh/sshd_config
```

Common regex issues:
- Forgetting to escape special characters: `.` `*` `+` `?`
- Anchors: Use `^` for start, `$` for end
- Whitespace: Use `\s+` for one or more spaces

### Line inserted in wrong place

Use `insertafter` or `insertbefore`:

```yaml
- lineinfile:
    path: /etc/config
    line: "new_setting = value"
    insertafter: '^# Settings section'
```

### Backrefs not working

Ensure `backrefs: yes` is set and regexp has capture groups:

```yaml
- lineinfile:
    path: /etc/config
    regexp: '^(version=).*$'
    line: '\g<1>2.0.0'
    backrefs: yes
```

### File permissions change after edit

Specify owner, group, and mode:

```yaml
- lineinfile:
    path: /etc/secure.conf
    line: "secret_key = value"
    owner: root
    group: root
    mode: "0600"
```

### Create not working

Ensure `create: yes` is set:

```yaml
- lineinfile:
    path: /etc/newfile.conf
    line: "first_line"
    create: yes
```

### State absent not removing line

Check your regexp matches the actual line:

```yaml
- name: Remove all matching lines
  lineinfile:
    path: /etc/config
    regexp: '.*old_pattern.*'
    state: absent
```

### File encoding issues

Ensure the file is UTF-8 encoded:

```bash
file /etc/config
iconv -f ISO-8859-1 -t UTF-8 /etc/config > /etc/config.utf8
```

## See Also

- [blockinfile](blockinfile.md) - Manage blocks of text in files
- [copy](copy.md) - Copy files
- [template](template.md) - Template files
- [file](file.md) - Manage file permissions
- [stat](stat.md) - Check file before modifying
