# user - Manage User Accounts

## Synopsis

The `user` module manages user accounts on Unix-like systems. It can create, modify, and remove user accounts, manage groups, and set up SSH authorized keys.

## Classification

**RemoteCommand** - This module executes user management commands on remote hosts via SSH.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `name` | yes | - | string | Name of the user account. |
| `state` | no | present | string | Desired state: present, absent. |
| `uid` | no | - | integer | User ID to assign. |
| `group` | no | - | string | Primary group of the user. |
| `groups` | no | - | list | List of supplementary groups. |
| `append` | no | false | boolean | Append groups rather than replace. |
| `shell` | no | - | string | Login shell for the user. |
| `home` | no | - | string | Home directory path. |
| `create_home` | no | true | boolean | Create home directory if it does not exist. |
| `move_home` | no | false | boolean | Move home directory if it changes. |
| `system` | no | false | boolean | Create a system account. |
| `comment` | no | - | string | GECOS field (user description). |
| `password` | no | - | string | Encrypted password. |
| `password_lock` | no | - | boolean | Lock the password. |
| `expires` | no | - | float | Account expiration time (epoch). |
| `remove` | no | false | boolean | Remove home directory when state=absent. |
| `force` | no | false | boolean | Force removal even if user is logged in. |
| `ssh_key_file` | no | - | string | Path to SSH authorized_keys file. |
| `ssh_key_type` | no | rsa | string | SSH key type to generate. |
| `ssh_key_bits` | no | - | integer | Number of bits for SSH key. |
| `ssh_key_comment` | no | - | string | Comment for generated SSH key. |
| `generate_ssh_key` | no | false | boolean | Generate SSH key for the user. |

## State Values

| State | Description |
|-------|-------------|
| `present` | Ensure the user exists |
| `absent` | Ensure the user does not exist |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `name` | string | User name |
| `uid` | integer | User ID |
| `group` | string | Primary group |
| `groups` | list | Supplementary groups |
| `home` | string | Home directory path |
| `shell` | string | Login shell |
| `ssh_key_file` | string | Path to SSH key file |

## Examples

### Create a user

```yaml
- name: Create user john
  user:
    name: john
    state: present
```

### Create a user with specific settings

```yaml
- name: Create app user
  user:
    name: appuser
    uid: 1500
    group: appgroup
    shell: /bin/bash
    home: /opt/app
    comment: "Application User"
```

### Create a system user

```yaml
- name: Create system user for service
  user:
    name: myservice
    system: yes
    shell: /sbin/nologin
    create_home: no
```

### Add user to supplementary groups

```yaml
- name: Add user to groups
  user:
    name: john
    groups:
      - docker
      - wheel
    append: yes
```

### Remove a user

```yaml
- name: Remove user
  user:
    name: olduser
    state: absent
```

### Remove user and home directory

```yaml
- name: Completely remove user
  user:
    name: olduser
    state: absent
    remove: yes
    force: yes
```

### Generate SSH key for user

```yaml
- name: Create user with SSH key
  user:
    name: deploy
    generate_ssh_key: yes
    ssh_key_bits: 4096
    ssh_key_comment: "deploy@example.com"
```

### Lock user password

```yaml
- name: Lock user account
  user:
    name: john
    password_lock: yes
```

### Set account expiration

```yaml
- name: Set account to expire
  user:
    name: contractor
    expires: 1735689600  # 2025-01-01
```

## Notes

- The `password` parameter expects a pre-hashed password (use `password_hash` filter)
- Use `append: yes` to add groups without removing existing group memberships
- System accounts are created with lower UIDs and no aging information
- The `remove` option only works with `state: absent`
- SSH keys are stored in `~/.ssh/` by default

## See Also

- [group](group.md) - Manage system groups
