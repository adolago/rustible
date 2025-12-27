# dnf - Manage DNF Packages

## Synopsis

The `dnf` module manages packages on Fedora and RHEL 8+ systems using the DNF package manager. DNF is the next-generation version of YUM. This module supports individual packages, package groups (using `@group` syntax), security/bugfix updates, repository management, and alternate installation roots.

## Classification

**RemoteCommand** - This module executes DNF commands on remote hosts via SSH.

## Parallelization

**HostExclusive** - Only one DNF operation can run per host at a time to prevent lock conflicts.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `name` | yes | - | string/list | Package name(s) or group(s) to manage. Use `@groupname` for groups. |
| `state` | no | present | string | Desired state: present, absent, latest, installed, removed. |
| `enablerepo` | no | - | string | Repository to enable for this operation. |
| `disablerepo` | no | - | string | Repository to disable for this operation. |
| `disable_gpg_check` | no | false | boolean | Disable GPG signature checking. |
| `update_cache` | no | false | boolean | Force dnf to check if cache is out of date. |
| `security` | no | false | boolean | Only install security updates. |
| `bugfix` | no | false | boolean | Only install bugfix updates. |
| `exclude` | no | - | string | Package names to exclude from updates (supports wildcards). |
| `installroot` | no | / | string | Alternate installation root directory. |
| `releasever` | no | - | string | Set the release version for repositories. |
| `allowerasing` | no | false | boolean | Allow erasing of installed packages to resolve dependencies. |
| `nobest` | no | false | boolean | Do not limit transactions to best candidate. |

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
| `rc` | integer | Return code from dnf |
| `results` | object | Map of package names to their status (installed, removed, ok) |

## Examples

### Install a package

```yaml
- name: Install nginx
  dnf:
    name: nginx
    state: present
```

### Install multiple packages

```yaml
- name: Install development tools
  dnf:
    name:
      - gcc
      - make
      - kernel-devel
    state: present
```

### Install a package group

```yaml
- name: Install Development Tools group
  dnf:
    name: "@Development Tools"
    state: present
```

### Install mixed packages and groups

```yaml
- name: Install packages and groups
  dnf:
    name:
      - nginx
      - "@Web Server"
      - "@PHP Support"
    state: present
```

### Remove a package

```yaml
- name: Remove unused package
  dnf:
    name: telnet
    state: absent
```

### Remove a package group

```yaml
- name: Remove development group
  dnf:
    name: "@Development Tools"
    state: absent
```

### Upgrade a package

```yaml
- name: Upgrade nginx to latest
  dnf:
    name: nginx
    state: latest
```

### Install from a specific repository

```yaml
- name: Install from EPEL
  dnf:
    name: htop
    state: present
    enablerepo: epel
```

### Install while disabling a repository

```yaml
- name: Install without updates repo
  dnf:
    name: httpd
    state: present
    disablerepo: updates
```

### Install security updates only

```yaml
- name: Apply security updates
  dnf:
    name: '*'
    state: latest
    security: yes
```

### Install bugfix updates only

```yaml
- name: Apply bugfix updates
  dnf:
    name: '*'
    state: latest
    bugfix: yes
```

### Exclude packages from update

```yaml
- name: Update all except kernel
  dnf:
    name: '*'
    state: latest
    exclude: 'kernel*'
```

### Install with dependency resolution

```yaml
- name: Install with erasing conflicting packages
  dnf:
    name: new-package
    state: present
    allowerasing: yes
```

### Install to alternate root

```yaml
- name: Install to chroot environment
  dnf:
    name: bash
    state: present
    installroot: /mnt/sysimage
```

### Install with specific release version

```yaml
- name: Install for specific release
  dnf:
    name: httpd
    state: present
    releasever: "8"
```

### Upgrade all packages

```yaml
- name: Upgrade all packages
  dnf:
    name: '*'
    state: latest
    update_cache: yes
```

### Install without best candidate restriction

```yaml
- name: Install allowing older versions
  dnf:
    name: package-name
    state: present
    nobest: yes
```

## Diff Mode

When running in diff mode, the module provides detailed information about changes:

- For `state: present`: Shows if packages will be installed
- For `state: latest`: Shows current version and available version (e.g., `nginx: 1.18.0 -> 1.20.0`)
- For `state: absent`: Shows packages that will be removed
- For groups: Shows group installation status

## Notes

- DNF is the default package manager on Fedora and RHEL 8+
- For RHEL/CentOS 7, use the `yum` module instead
- Package names are validated to prevent command injection
- Package groups must be prefixed with `@` (e.g., `@Development Tools`)
- The `allowerasing` option can resolve complex dependency conflicts
- The `nobest` option allows installing older versions when needed
- Repository enabling/disabling is temporary for the operation only
- Security and bugfix filters can be combined with other options

## See Also

- [package](package.md) - Generic package manager
- [yum](yum.md) - RHEL/CentOS 7 package management
- [apt](apt.md) - Debian/Ubuntu package management
