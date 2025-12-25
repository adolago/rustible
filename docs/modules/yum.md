# yum - Manage YUM Packages

## Synopsis

The `yum` module manages packages on Red Hat-based systems using the YUM package manager. It is primarily used on RHEL/CentOS 7 and earlier versions. This module supports individual packages, package groups (using `@group` syntax), security/bugfix updates, repository management, and alternate installation roots.

## Classification

**RemoteCommand** - This module executes YUM commands on remote hosts via SSH.

## Parallelization

**HostExclusive** - Only one YUM operation can run per host at a time to prevent lock conflicts.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `name` | yes | - | string/list | Package name(s) or group(s) to manage. Use `@groupname` for groups. |
| `state` | no | present | string | Desired state: present, absent, latest, installed, removed. |
| `enablerepo` | no | - | string | Repository to enable for this operation. |
| `disablerepo` | no | - | string | Repository to disable for this operation. |
| `disable_gpg_check` | no | false | boolean | Disable GPG signature checking. |
| `update_cache` | no | false | boolean | Force yum to check for updated cache. |
| `security` | no | false | boolean | Only install security updates. |
| `bugfix` | no | false | boolean | Only install bugfix updates. |
| `exclude` | no | - | string | Package names to exclude from updates (supports wildcards). |
| `installroot` | no | / | string | Alternate installation root directory. |
| `releasever` | no | - | string | Set the release version for repositories. |

## State Values

| State | Description |
|-------|-------------|
| `present` or `installed` | Ensure the package is installed |
| `absent` or `removed` | Ensure the package is removed |
| `latest` | Ensure the package is at the latest version |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `msg` | string | Status message |
| `rc` | integer | Return code from yum |
| `results` | object | Map of package names to their status (installed, removed, ok) |

## Examples

### Install a package

```yaml
- name: Install httpd
  yum:
    name: httpd
    state: present
```

### Install multiple packages

```yaml
- name: Install LAMP stack
  yum:
    name:
      - httpd
      - mariadb-server
      - php
    state: present
```

### Install a package group

```yaml
- name: Install Development Tools group
  yum:
    name: "@Development Tools"
    state: present
```

### Install mixed packages and groups

```yaml
- name: Install packages and groups
  yum:
    name:
      - httpd
      - "@Web Server"
      - "@PHP Support"
    state: present
```

### Remove a package

```yaml
- name: Remove telnet
  yum:
    name: telnet
    state: absent
```

### Remove a package group

```yaml
- name: Remove development group
  yum:
    name: "@Development Tools"
    state: absent
```

### Upgrade a package

```yaml
- name: Upgrade httpd to latest
  yum:
    name: httpd
    state: latest
```

### Install from a specific repository

```yaml
- name: Install from EPEL
  yum:
    name: nginx
    state: present
    enablerepo: epel
```

### Install while disabling a repository

```yaml
- name: Install without updates repo
  yum:
    name: httpd
    state: present
    disablerepo: updates
```

### Install security updates only

```yaml
- name: Apply security updates
  yum:
    name: '*'
    state: latest
    security: yes
```

### Install bugfix updates only

```yaml
- name: Apply bugfix updates
  yum:
    name: '*'
    state: latest
    bugfix: yes
```

### Install with GPG check disabled

```yaml
- name: Install unsigned package
  yum:
    name: custom-package
    state: present
    disable_gpg_check: yes
```

### Exclude packages from update

```yaml
- name: Update all except kernel
  yum:
    name: '*'
    state: latest
    exclude: 'kernel*'
```

### Install to alternate root

```yaml
- name: Install to chroot environment
  yum:
    name: bash
    state: present
    installroot: /mnt/sysimage
```

### Install with specific release version

```yaml
- name: Install for specific release
  yum:
    name: httpd
    state: present
    releasever: "7"
```

### Upgrade all packages

```yaml
- name: Upgrade all packages
  yum:
    name: '*'
    state: latest
    update_cache: yes
```

## Diff Mode

When running in diff mode, the module provides detailed information about changes:

- For `state: present`: Shows if packages will be installed
- For `state: latest`: Shows current version and available version (e.g., `httpd: 2.4.6 -> 2.4.51`)
- For `state: absent`: Shows packages that will be removed
- For groups: Shows group installation status

## Notes

- For RHEL/CentOS 8 and later, use the `dnf` module instead
- Package names are validated to prevent command injection
- Package groups must be prefixed with `@` (e.g., `@Development Tools`)
- The `*` wildcard can be used with `state: latest` to upgrade all packages
- Repository enabling/disabling is temporary for the operation only
- Security and bugfix filters can be combined with other options
- The yum module uses `groupinstall`/`groupremove` for package groups

## See Also

- [package](package.md) - Generic package manager
- [dnf](dnf.md) - Fedora/RHEL 8+ package management
- [apt](apt.md) - Debian/Ubuntu package management
