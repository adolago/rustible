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

## See Also

- [file](file.md) - Manage file properties
- [assert](assert.md) - Assert conditions
