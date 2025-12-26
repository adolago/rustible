# group - Manage System Groups

## Synopsis

The `group` module manages groups on Unix-like systems. It can create, modify, and remove groups.

## Classification

**RemoteCommand** - This module executes group management commands on remote hosts via SSH.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `name` | yes | - | string | Name of the group. |
| `state` | no | present | string | Desired state: present, absent. |
| `gid` | no | - | integer | Group ID to assign. |
| `system` | no | false | boolean | Create a system group. |
| `local` | no | false | boolean | Force use of local command alternatives. |
| `non_unique` | no | false | boolean | Allow non-unique GID. |

## State Values

| State | Description |
|-------|-------------|
| `present` | Ensure the group exists |
| `absent` | Ensure the group does not exist |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `name` | string | Group name |
| `gid` | integer | Group ID |
| `system` | boolean | Whether it is a system group |
| `state` | string | Current state |

## Examples

### Create a group

```yaml
- name: Create developers group
  group:
    name: developers
    state: present
```

### Create a group with specific GID

```yaml
- name: Create group with GID
  group:
    name: appgroup
    gid: 1500
    state: present
```

### Create a system group

```yaml
- name: Create system group
  group:
    name: myservice
    system: yes
    state: present
```

### Remove a group

```yaml
- name: Remove old group
  group:
    name: oldgroup
    state: absent
```

### Create group before user

```yaml
- name: Create application group
  group:
    name: webapp
    gid: 2000

- name: Create application user in group
  user:
    name: webuser
    group: webapp
```

## Notes

- Groups cannot be removed if users still belong to them as their primary group
- System groups are typically created with GIDs less than 1000
- The `non_unique` option allows duplicate GIDs (use with caution)
- Use the `user` module to add users to groups

## Real-World Use Cases

### Application Groups

```yaml
- name: Create application groups
  group:
    name: "{{ item }}"
    state: present
  loop:
    - webapps
    - deployers
    - monitoring
```

### System Service Group

```yaml
- name: Create service group
  group:
    name: myservice
    system: yes
    gid: 999

- name: Create service user in group
  user:
    name: myservice
    group: myservice
    system: yes
```

### Shared Access Group

```yaml
- name: Create shared data group
  group:
    name: shared-data
    gid: 2000

- name: Add users to shared group
  user:
    name: "{{ item }}"
    groups: shared-data
    append: yes
  loop:
    - user1
    - user2
    - user3
```

## Troubleshooting

### Cannot remove group - in use

The group is a primary group for one or more users:

```bash
# Find users with this primary group
grep :groupname: /etc/group
getent passwd | awk -F: '{print $1, $4}'
```

Change users' primary groups first:

```yaml
- name: Change user's primary group
  user:
    name: affecteduser
    group: newgroup

- name: Now remove old group
  group:
    name: oldgroup
    state: absent
```

### GID conflict

Another group already uses the GID:

```bash
getent group 1500
```

Solutions:
1. Use a different GID
2. Remove the conflicting group
3. Use `non_unique: yes` (not recommended)

```yaml
- group:
    name: mygroup
    gid: 1501  # Different GID
```

### Group changes not taking effect

Users must log out and log back in for group changes to take effect:

```bash
# Verify group membership
id username
groups username
```

Or use `newgrp` to activate new group in current session:

```bash
newgrp docker
```

### System group has wrong GID range

System groups should have GIDs < 1000:

```yaml
- group:
    name: myservice
    system: yes  # Assigns GID < 1000
```

### Group exists with different GID

Cannot change GID of existing group. Must remove and recreate:

```yaml
# First change any users using this as primary group
- user:
    name: existinguser
    group: users

- group:
    name: mygroup
    state: absent

- group:
    name: mygroup
    gid: 1500
    state: present
```

## See Also

- [user](user.md) - Manage user accounts
- [file](file.md) - Set group ownership on files
- [acl](acl.md) - Set access control lists for group access
