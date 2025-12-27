# apt - Manage APT Packages

## Synopsis

The `apt` module manages packages on Debian-based systems using the APT package manager. It provides full Ansible compatibility with support for installing, removing, and upgrading packages, managing the package cache, installing from `.deb` files, and performing system upgrades.

## Classification

**RemoteCommand** - This module executes APT commands on remote hosts via SSH.

## Parallelization

**HostExclusive** - Only one APT operation can run per host at a time to prevent lock conflicts.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `name` | no* | - | string/list | Package name(s) to manage. Supports version specifiers (e.g., `nginx=1.18.0`). Aliases: `package`, `pkg` |
| `state` | no | present | string | Desired state: present, absent, latest, build-dep, fixed |
| `update_cache` | no | false | boolean | Run `apt-get update` before the operation. Alias: `update-cache` |
| `cache_valid_time` | no | 0 | integer | Time in seconds the cache is valid. If set, implies `update_cache=true`. Skip update if cache is recent enough. |
| `upgrade` | no | - | string | Upgrade type: dist, full, yes, safe. When set, performs a system-wide upgrade. |
| `autoremove` | no | false | boolean | Remove unused dependency packages after operation. |
| `autoclean` | no | false | boolean | Clean the apt cache after operation. |
| `purge` | no | false | boolean | Remove package configuration files when removing (purge instead of remove). |
| `force` | no | false | boolean | Force operations. Enables `--allow-unauthenticated`, `--allow-downgrades`, `--allow-remove-essential`, and `--allow-change-held-packages`. |
| `deb` | no | - | string | Path to a local `.deb` file to install. |
| `default_release` | no | - | string | Default release for apt pinning using `-t` option (e.g., "buster-backports"). Alias: `default-release` |
| `install_recommends` | no | - | boolean | Install recommended packages. If not set, uses system default. Set to `false` for minimal installations. Alias: `install-recommends` |
| `dpkg_options` | no | force-confdef,force-confold | string | Comma-separated list of dpkg options to pass to apt-get. |
| `allow_downgrade` | no | false | boolean | Allow replacing packages with lower versions. |
| `allow_unauthenticated` | no | false | boolean | Allow installing unauthenticated packages. |
| `only_upgrade` | no | false | boolean | Only upgrade packages that are already installed (do not install new packages). |
| `force_apt_get` | no | false | boolean | Force use of apt-get instead of aptitude. |

*At least one of `name`, `upgrade`, `deb`, or `update_cache` must be specified.

## State Values

| State | Description |
|-------|-------------|
| `present` | Ensure the package is installed (any version) |
| `absent` | Ensure the package is removed |
| `latest` | Ensure the package is at the latest available version |
| `build-dep` | Install build dependencies for the package |
| `fixed` | Attempt to correct broken dependencies |

## Upgrade Modes

| Mode | Description |
|------|-------------|
| `yes` / `safe` | Perform a safe upgrade (apt-get upgrade) |
| `dist` / `full` | Perform a distribution upgrade (apt-get dist-upgrade) |
| `no` | No upgrade (default) |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `cache_updated` | boolean | Whether the cache was updated |
| `packages` | object | Map of package names to their result status |
| `stdout` | string | Standard output from apt commands |
| `stderr` | string | Standard error from apt commands |

## Examples

### Install a package

```yaml
- name: Install nginx
  apt:
    name: nginx
    state: present
```

### Install multiple packages

```yaml
- name: Install web stack
  apt:
    name:
      - nginx
      - php-fpm
      - mariadb-server
    state: present
    update_cache: yes
```

### Install a specific version

```yaml
- name: Install specific nginx version
  apt:
    name: nginx=1.18.0-0ubuntu1
    state: present
```

### Remove a package

```yaml
- name: Remove old package
  apt:
    name: apache2
    state: absent
```

### Remove a package and its configuration (purge)

```yaml
- name: Purge mysql-server
  apt:
    name: mysql-server
    state: absent
    purge: yes
```

### Update cache with validity time

```yaml
- name: Update cache if older than 1 hour
  apt:
    name: htop
    state: present
    update_cache: yes
    cache_valid_time: 3600
```

### Just update the cache

```yaml
- name: Update apt cache
  apt:
    update_cache: yes
```

### Upgrade all packages (safe)

```yaml
- name: Safe upgrade all packages
  apt:
    upgrade: yes
    update_cache: yes
```

### Distribution upgrade

```yaml
- name: Dist-upgrade all packages
  apt:
    upgrade: dist
    update_cache: yes
```

### Install from a .deb file

```yaml
- name: Install local package
  apt:
    deb: /tmp/mypackage.deb
```

### Install build dependencies

```yaml
- name: Install build deps for nginx
  apt:
    name: nginx
    state: build-dep
```

### Minimal install without recommends

```yaml
- name: Minimal package installation
  apt:
    name: some-package
    state: present
    install_recommends: no
```

### Install from backports

```yaml
- name: Install from backports
  apt:
    name: nginx
    state: present
    default_release: buster-backports
```

### Clean up unused packages

```yaml
- name: Remove unused dependencies
  apt:
    autoremove: yes
    purge: yes
```

### Fix broken dependencies

```yaml
- name: Fix broken packages
  apt:
    state: fixed
```

### Force installation with downgrades

```yaml
- name: Force install with potential downgrade
  apt:
    name: some-package
    state: present
    allow_downgrade: yes
```

### Complete system maintenance

```yaml
- name: Full system update and cleanup
  apt:
    upgrade: dist
    update_cache: yes
    autoremove: yes
    autoclean: yes
```

## Check Mode Support

The apt module fully supports check mode (`--check`). In check mode, the module will:

- Report what packages would be installed, removed, or upgraded
- Report if the cache would be updated
- Report if autoremove would remove packages
- Not make any actual changes to the system

Example output in check mode:
```
Would update cache. Would install: nginx, htop. Would autoremove unused packages.
```

## Diff Mode Support

The apt module supports diff mode (`--diff`). In diff mode, the module provides detailed before/after comparisons showing:

- Current installed version vs. target version
- Whether packages will be installed, upgraded, or removed
- Cache update status
- Autoremove impact

## Notes

- Package names are validated against a safe regex pattern to prevent command injection
- The module uses `apt-get` commands internally with `DEBIAN_FRONTEND=noninteractive`
- The `cache_valid_time` option helps reduce unnecessary cache updates in frequently-run playbooks
- The `autoremove` option is useful after removing packages with many dependencies
- Version specifiers use the format `package=version` (e.g., `nginx=1.18.0-0ubuntu1`)
- The `dpkg_options` default of `force-confdef,force-confold` prevents interactive prompts during upgrades

## Security Considerations

- Package names are validated to contain only safe characters (`[a-zA-Z0-9._+-]+`)
- The `force` option should be used with caution as it bypasses security checks
- The `allow_unauthenticated` option bypasses GPG signature verification
- All commands are properly escaped to prevent shell injection

## Real-World Use Cases

### Web Server Stack

```yaml
- name: Install LAMP stack
  apt:
    name:
      - apache2
      - mariadb-server
      - php
      - php-mysql
      - php-curl
      - libapache2-mod-php
    state: present
    update_cache: yes
    cache_valid_time: 3600
```

### Docker Installation

```yaml
- name: Install Docker prerequisites
  apt:
    name:
      - apt-transport-https
      - ca-certificates
      - curl
      - gnupg
      - lsb-release
    state: present
    update_cache: yes

- name: Install Docker
  apt:
    name:
      - docker-ce
      - docker-ce-cli
      - containerd.io
    state: present
```

### Security Updates Only

```yaml
- name: Install security updates
  apt:
    upgrade: yes
    update_cache: yes
  environment:
    DEBIAN_FRONTEND: noninteractive
  when: "'security' in ansible_upgrade_results"
```

## Troubleshooting

### dpkg lock error

Another process is using dpkg. Wait for it to finish or investigate:

```bash
# Check for lock
lsof /var/lib/dpkg/lock-frontend

# Wait and retry
sudo fuser -v /var/lib/dpkg/lock-frontend
```

Solution: Use a wait loop in your playbook:

```yaml
- name: Wait for apt lock
  shell: while fuser /var/lib/dpkg/lock-frontend >/dev/null 2>&1; do sleep 5; done
  changed_when: false
```

### Package not found

Update the cache and check package name:

```yaml
- name: Install with cache update
  apt:
    name: package-name
    state: present
    update_cache: yes
```

Check if package exists:
```bash
apt-cache search package-name
apt-cache policy package-name
```

### Version specification not working

Use exact version string from `apt-cache policy`:

```bash
apt-cache policy nginx
# Use the exact version shown
```

```yaml
- apt:
    name: nginx=1.18.0-0ubuntu1.4
    state: present
```

### Held packages preventing upgrade

Check for held packages:
```bash
apt-mark showhold
```

Either unhold or use force:
```yaml
- apt:
    name: held-package
    state: latest
    force: yes  # Use with caution
```

### Interactive prompts causing failure

Ensure DEBIAN_FRONTEND is set (automatic in module):

```yaml
- apt:
    name: package
    dpkg_options: "force-confdef,force-confold"
```

### Broken dependencies

Try fixing first:
```yaml
- name: Fix broken packages
  apt:
    state: fixed
```

### GPG key errors

Add repository keys before using the repository:
```yaml
- name: Add GPG key
  apt_key:
    url: https://example.com/key.gpg
    state: present

- name: Add repository
  apt_repository:
    repo: deb https://example.com/repo stable main
    state: present
```

## See Also

- [package](package.md) - Generic package manager (auto-detects apt/yum/dnf)
- [yum](yum.md) - RHEL/CentOS package management
- [dnf](dnf.md) - Fedora package management
- [pip](pip.md) - Python package management
- [service](service.md) - Manage services after package installation
- [command](command.md) - Run post-install commands
