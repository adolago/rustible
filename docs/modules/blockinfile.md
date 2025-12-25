# blockinfile - Manage Blocks of Text in Files

## Synopsis

The `blockinfile` module inserts, updates, or removes a block of multi-line text surrounded by customizable marker lines. It is useful for managing configuration sections.

## Classification

**NativeTransport** - This module uses native Rust operations for file manipulation and SSH/SFTP for transfer.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `path` | yes | - | string | Path to the file to modify. |
| `block` | no | "" | string | The block of text to insert (empty removes block). |
| `state` | no | present | string | Desired state: present, absent. |
| `marker` | no | "# {mark} ANSIBLE MANAGED BLOCK" | string | Marker template. Use {mark} placeholder. |
| `marker_begin` | no | BEGIN | string | Text for the beginning marker. |
| `marker_end` | no | END | string | Text for the ending marker. |
| `insertafter` | no | EOF | string | Insert after this regex or EOF. |
| `insertbefore` | no | - | string | Insert before this regex or BOF. |
| `create` | no | false | boolean | Create file if it does not exist. |
| `backup` | no | false | boolean | Create backup before modifying. |
| `owner` | no | - | string | Owner of the file. |
| `group` | no | - | string | Group of the file. |
| `mode` | no | - | string | Permissions of the file. |

## State Values

| State | Description |
|-------|-------------|
| `present` | Ensure the block is in the file |
| `absent` | Remove the block from the file |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `changed` | boolean | Whether the file was modified |
| `msg` | string | Status message |
| `backup` | string | Path to backup file (if created) |

## Examples

### Insert a block of configuration

```yaml
- name: Add custom configuration block
  blockinfile:
    path: /etc/nginx/nginx.conf
    block: |
      # Custom upstream configuration
      upstream backend {
          server 127.0.0.1:8080;
          server 127.0.0.1:8081;
      }
    marker: "# {mark} MANAGED UPSTREAM CONFIG"
```

### Insert SSH config block

```yaml
- name: Add SSH config for bastion
  blockinfile:
    path: ~/.ssh/config
    block: |
      Host bastion
          HostName bastion.example.com
          User admin
          IdentityFile ~/.ssh/bastion_key
    create: yes
    mode: '0600'
```

### Remove a managed block

```yaml
- name: Remove old configuration
  blockinfile:
    path: /etc/myapp/config.conf
    marker: "# {mark} OLD CONFIG BLOCK"
    state: absent
```

### Insert after specific content

```yaml
- name: Add after server block
  blockinfile:
    path: /etc/nginx/sites-available/default
    insertafter: 'server {'
    block: |
        # Rate limiting
        limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;
```

### Use custom markers

```yaml
- name: Add with custom markers
  blockinfile:
    path: /etc/apache2/apache2.conf
    block: |
      <Directory /var/www/app>
          AllowOverride All
      </Directory>
    marker: "### {mark} APP DIRECTORY CONFIG ###"
    marker_begin: "START"
    marker_end: "FINISH"
```

### Create file with block

```yaml
- name: Create config with initial block
  blockinfile:
    path: /etc/myapp/settings.conf
    block: |
      [database]
      host = localhost
      port = 5432
    create: yes
    owner: root
    group: root
    mode: '0644'
```

### Update existing block

```yaml
- name: Update managed block
  blockinfile:
    path: /etc/hosts
    block: |
      192.168.1.10 server1
      192.168.1.11 server2
      192.168.1.12 server3
    marker: "# {mark} CUSTOM HOSTS"
```

## Notes

- The block is surrounded by marker lines that identify the managed section
- If the markers exist, the block between them is replaced
- If the markers do not exist, the block is inserted at the specified position
- An empty `block` or `state: absent` removes the managed section
- The `{mark}` placeholder in `marker` is replaced with BEGIN/END (or custom values)
- Multiple managed blocks can exist in the same file with different markers

## See Also

- [lineinfile](lineinfile.md) - Manage single lines in files
- [template](template.md) - Template entire files
- [copy](copy.md) - Copy files
