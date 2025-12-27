# stat - Retrieve File Information

## Synopsis

The `stat` module retrieves file or directory status information. It returns detailed information about files, directories, and symbolic links including size, permissions, ownership, and timestamps.

## Classification

**NativeTransport** - This module uses native Rust operations for file system access.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `path` | yes | - | string | Path to the file or directory to stat. |
| `follow` | no | true | boolean | Follow symbolic links. |
| `checksum` | no | false | boolean | Calculate file checksum. |
| `checksum_algorithm` | no | sha1 | string | Algorithm: md5, sha1, sha256. |

## Return Values

The module returns data in the `stat` key with the following fields:

| Key | Type | Description |
|-----|------|-------------|
| `exists` | boolean | Whether the path exists |
| `path` | string | The path that was checked |
| `isdir` | boolean | Whether it is a directory |
| `isreg` | boolean | Whether it is a regular file |
| `islnk` | boolean | Whether it is a symbolic link |
| `mode` | string | File permissions in octal |
| `size` | integer | Size in bytes |
| `uid` | integer | Owner user ID |
| `gid` | integer | Owner group ID |
| `atime` | integer | Last access time (epoch) |
| `mtime` | integer | Last modification time (epoch) |
| `readable` | boolean | Whether file is readable |
| `writeable` | boolean | Whether file is writable |
| `executable` | boolean | Whether file is executable |
| `checksum` | string | File checksum (if requested) |
| `lnk_source` | string | Symlink target (if symlink and follow=true) |

## Examples

### Check if a file exists

```yaml
- name: Check if config exists
  stat:
    path: /etc/myapp/config.yml
  register: config_stat

- name: Create config if missing
  template:
    src: config.yml.j2
    dest: /etc/myapp/config.yml
  when: not config_stat.stat.exists
```

### Get file checksum

```yaml
- name: Get file checksum
  stat:
    path: /etc/myapp/data.bin
    checksum: yes
    checksum_algorithm: sha256
  register: file_stat

- name: Show checksum
  debug:
    msg: "Checksum: {{ file_stat.stat.checksum }}"
```

### Check file permissions

```yaml
- name: Check file permissions
  stat:
    path: /etc/shadow
  register: shadow_stat

- name: Verify secure permissions
  assert:
    that:
      - shadow_stat.stat.mode == '0640'
    fail_msg: "Shadow file has incorrect permissions"
```

### Check if path is a directory

```yaml
- name: Check path type
  stat:
    path: /var/log/myapp
  register: path_stat

- name: Create directory if missing
  file:
    path: /var/log/myapp
    state: directory
  when: not path_stat.stat.exists or not path_stat.stat.isdir
```

### Check symlink target

```yaml
- name: Check symlink
  stat:
    path: /etc/alternatives/python
    follow: no
  register: link_stat

- name: Show symlink info
  debug:
    msg: "Link points to: {{ link_stat.stat.lnk_source }}"
  when: link_stat.stat.islnk
```

### Compare file modification times

```yaml
- name: Get source file info
  stat:
    path: /tmp/source.txt
  register: source_stat

- name: Get dest file info
  stat:
    path: /opt/dest.txt
  register: dest_stat

- name: Copy if source is newer
  copy:
    src: /tmp/source.txt
    dest: /opt/dest.txt
  when: >
    not dest_stat.stat.exists or
    source_stat.stat.mtime > dest_stat.stat.mtime
```

## Notes

- The `stat` module is read-only and never changes files
- In check mode, it behaves identically to normal mode
- The `follow` option determines whether symlinks are dereferenced
- Checksum calculation is disabled by default for performance
- Timestamps are returned as Unix epoch seconds
- Use `follow: no` to get information about the symlink itself

## Real-World Use Cases

### Conditional File Operations

```yaml
- name: Check if backup exists
  stat:
    path: /backup/database.sql.gz
  register: backup_stat

- name: Create backup if none exists
  shell: pg_dump mydb | gzip > /backup/database.sql.gz
  when: not backup_stat.stat.exists

- name: Verify backup is recent (< 24 hours)
  assert:
    that: (ansible_date_time.epoch | int) - backup_stat.stat.mtime < 86400
    fail_msg: "Backup is older than 24 hours"
  when: backup_stat.stat.exists
```

### Security Audit

```yaml
- name: Check critical file permissions
  stat:
    path: "{{ item }}"
  register: security_stat
  loop:
    - /etc/shadow
    - /etc/ssh/sshd_config
    - /root/.ssh/authorized_keys

- name: Verify secure permissions
  assert:
    that:
      - item.stat.mode == "0600" or item.stat.mode == "0640"
      - item.stat.uid == 0
    fail_msg: "{{ item.item }} has insecure permissions"
  loop: "{{ security_stat.results }}"
  when: item.stat.exists
```

### File Change Detection

```yaml
- name: Get config file checksum before change
  stat:
    path: /etc/myapp/config.yml
    checksum: yes
    checksum_algorithm: sha256
  register: config_before

# ... make changes ...

- name: Get config file checksum after change
  stat:
    path: /etc/myapp/config.yml
    checksum: yes
    checksum_algorithm: sha256
  register: config_after

- name: Restart if config changed
  service:
    name: myapp
    state: restarted
  when: config_before.stat.checksum != config_after.stat.checksum
```

### Disk Space Check

```yaml
- name: Get mount point info
  stat:
    path: /var/lib/docker
  register: docker_stat

- name: Ensure sufficient space
  assert:
    that: docker_stat.stat.blocks_available * 4096 > 10737418240
    fail_msg: "Less than 10GB available for Docker"
```

## Troubleshooting

### File does not exist

Check the `exists` field before accessing other attributes:

```yaml
- stat:
    path: /maybe/exists
  register: result

- debug:
    msg: "File size is {{ result.stat.size }}"
  when: result.stat.exists
```

### Permission denied

Use privilege escalation:

```yaml
- stat:
    path: /root/.bashrc
  become: yes
  register: result
```

### Symlink not resolved

By default, stat follows symlinks. To get info about the link itself:

```yaml
- stat:
    path: /usr/bin/python
    follow: no
  register: python_link
```

### Checksum calculation slow

Checksum is disabled by default. Only enable when needed:

```yaml
- stat:
    path: /large/file
    checksum: yes  # Takes time for large files
```

### Mode shows unexpected value

Mode is returned as a string in octal format:

```yaml
- stat:
    path: /etc/passwd
  register: result

- debug:
    msg: "Mode is {{ result.stat.mode }}"
  # Shows "0644" not "420" (decimal)
```

### Cannot access attributes on undefined

Always check existence first:

```yaml
# WRONG - fails if file doesn't exist
- debug:
    msg: "Size: {{ result.stat.size }}"

# CORRECT - check first
- debug:
    msg: "Size: {{ result.stat.size }}"
  when: result.stat.exists
```

## See Also

- [file](file.md) - Manage file properties
- [assert](assert.md) - Assert conditions
- [copy](copy.md) - Copy files with checksum verification
- [debug](debug.md) - Print stat results
- [command](command.md) - Alternative for complex stat operations
