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

## See Also

- [user](user.md) - Manage user accounts
