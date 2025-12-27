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

## Real-World Use Cases

### Firewall Rules

```yaml
- name: Add firewall rules block
  blockinfile:
    path: /etc/iptables/rules.v4
    marker: "# {mark} APPLICATION RULES"
    block: |
      -A INPUT -p tcp --dport 80 -j ACCEPT
      -A INPUT -p tcp --dport 443 -j ACCEPT
      -A INPUT -p tcp --dport 22 -j ACCEPT
    insertafter: '*filter'
  notify: Reload iptables
```

### Nginx Server Block

```yaml
- name: Add upstream servers
  blockinfile:
    path: /etc/nginx/conf.d/upstream.conf
    create: yes
    marker: "# {mark} BACKEND SERVERS"
    block: |
      upstream backend {
          {% for server in backend_servers %}
          server {{ server }}:8080;
          {% endfor %}
      }
  notify: Reload nginx
```

### SSH Client Configuration

```yaml
- name: Configure SSH for internal hosts
  blockinfile:
    path: ~/.ssh/config
    create: yes
    mode: "0600"
    marker: "# {mark} INTERNAL HOSTS"
    block: |
      Host *.internal.example.com
          User admin
          IdentityFile ~/.ssh/internal_key
          ProxyJump bastion.example.com
```

### Sudoers Configuration

```yaml
- name: Add application sudo rules
  blockinfile:
    path: /etc/sudoers.d/myapp
    create: yes
    mode: "0440"
    validate: visudo -cf %s
    marker: "# {mark} MYAPP RULES"
    block: |
      %developers ALL=(appuser) NOPASSWD: /opt/myapp/bin/*
      %operators ALL=(root) NOPASSWD: /bin/systemctl restart myapp
```

## Troubleshooting

### Block appears multiple times

Ensure marker is unique for each block:

```yaml
# WRONG - same marker for different blocks
- blockinfile:
    path: /etc/config
    block: "block 1"
    # Uses default marker

- blockinfile:
    path: /etc/config
    block: "block 2"
    # Uses same default marker - replaces block 1!

# CORRECT - unique markers
- blockinfile:
    path: /etc/config
    block: "block 1"
    marker: "# {mark} BLOCK ONE"

- blockinfile:
    path: /etc/config
    block: "block 2"
    marker: "# {mark} BLOCK TWO"
```

### Block not being replaced

Check that markers match exactly:

```yaml
# If file has "# BEGIN MY BLOCK" use matching marker
- blockinfile:
    path: /etc/config
    marker: "# {mark} MY BLOCK"  # Must match existing
    block: "new content"
```

### Insertion position not working

Use `insertafter` or `insertbefore` with regex:

```yaml
- blockinfile:
    path: /etc/config
    insertafter: '^# Global settings'
    block: |
      custom_setting1 = value1
      custom_setting2 = value2
```

### Marker visible in config files

For configs that don't support `#` comments:

```yaml
# For XML files
- blockinfile:
    path: /etc/app/config.xml
    marker: "<!-- {mark} MANAGED CONFIG -->"
    block: |
      <setting name="option1" value="value1"/>

# For INI files (standard # works)
- blockinfile:
    path: /etc/app/config.ini
    marker: "# {mark} MANAGED SECTION"
```

### Empty block not removing content

Use `state: absent` to remove:

```yaml
- blockinfile:
    path: /etc/config
    marker: "# {mark} OLD BLOCK"
    state: absent
```

### File permissions change

Specify permissions explicitly:

```yaml
- blockinfile:
    path: /etc/secure.conf
    block: "content"
    owner: root
    group: root
    mode: "0640"
```

### Block adds extra blank lines

Use `|` for literal blocks and trim whitespace:

```yaml
- blockinfile:
    path: /etc/config
    marker: "# {mark} CONFIG"
    block: |
      line1
      line2
      line3
```

## See Also

- [lineinfile](lineinfile.md) - Manage single lines in files
- [template](template.md) - Template entire files
- [copy](copy.md) - Copy files
- [file](file.md) - Set file permissions
- [stat](stat.md) - Check file state before modifying
