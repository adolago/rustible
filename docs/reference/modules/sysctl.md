# sysctl - Manage Kernel Parameters

## Synopsis

The `sysctl` module manages kernel parameters via sysctl. It can set runtime parameters and optionally persist them in configuration files for survival across reboots. This is essential for performance tuning, security hardening, and network optimization.

## Classification

**RemoteCommand** - This module executes sysctl commands on remote hosts via SSH.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `name` | yes | - | string | Sysctl parameter name (e.g., net.ipv4.ip_forward). |
| `value` | conditional | - | string | Value to set. Required when state is present. |
| `state` | no | present | string | Desired state: present, absent. |
| `sysctl_file` | no | /etc/sysctl.d/99-rustible.conf | string | Configuration file for persistent settings. |
| `reload` | no | true | boolean | Reload sysctl after modifying configuration file. |
| `ignoreerrors` | no | false | boolean | Ignore errors when setting runtime values. |

## State Values

| State | Description |
|-------|-------------|
| `present` | Ensure the parameter is set to the specified value |
| `absent` | Remove the parameter from the configuration file |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `name` | string | Parameter name |
| `value` | string | Value that was set |
| `previous_value` | string | Previous value (if any) |
| `changed` | boolean | Whether changes were made |

## Examples

### Enable IP forwarding

```yaml
- name: Enable IP forwarding
  sysctl:
    name: net.ipv4.ip_forward
    value: "1"
    state: present
```

### Set multiple kernel parameters

```yaml
- name: Configure network performance settings
  sysctl:
    name: "{{ item.name }}"
    value: "{{ item.value }}"
  loop:
    - { name: net.core.somaxconn, value: "65535" }
    - { name: net.core.netdev_max_backlog, value: "65535" }
    - { name: net.ipv4.tcp_max_syn_backlog, value: "65535" }
```

### Configure memory management

```yaml
- name: Set swappiness for database server
  sysctl:
    name: vm.swappiness
    value: "10"

- name: Set dirty page ratio
  sysctl:
    name: vm.dirty_ratio
    value: "40"
```

### Security hardening

```yaml
- name: Disable ICMP redirects
  sysctl:
    name: "{{ item }}"
    value: "0"
  loop:
    - net.ipv4.conf.all.accept_redirects
    - net.ipv4.conf.default.accept_redirects
    - net.ipv6.conf.all.accept_redirects
    - net.ipv6.conf.default.accept_redirects

- name: Enable source address verification
  sysctl:
    name: net.ipv4.conf.all.rp_filter
    value: "1"
```

### Remove a parameter

```yaml
- name: Remove custom setting
  sysctl:
    name: net.ipv4.tcp_slow_start_after_idle
    state: absent
```

### Use custom configuration file

```yaml
- name: Set in custom sysctl file
  sysctl:
    name: fs.file-max
    value: "2097152"
    sysctl_file: /etc/sysctl.d/50-file-limits.conf
```

### Set without reloading

```yaml
- name: Set without immediate reload
  sysctl:
    name: kernel.pid_max
    value: "4194304"
    reload: no
```

### Ignore runtime errors

```yaml
- name: Set parameter that may not exist
  sysctl:
    name: net.ipv4.tcp_available_congestion_control
    value: bbr
    ignoreerrors: yes
```

## Real-World Use Cases

### High-Performance Web Server

```yaml
- name: Configure kernel for web server
  sysctl:
    name: "{{ item.name }}"
    value: "{{ item.value }}"
  loop:
    # TCP performance tuning
    - { name: net.core.somaxconn, value: "65535" }
    - { name: net.core.netdev_max_backlog, value: "65535" }
    - { name: net.ipv4.tcp_max_syn_backlog, value: "65535" }
    - { name: net.ipv4.tcp_fin_timeout, value: "15" }
    - { name: net.ipv4.tcp_tw_reuse, value: "1" }
    - { name: net.ipv4.tcp_keepalive_time, value: "300" }
    - { name: net.ipv4.tcp_keepalive_probes, value: "5" }
    - { name: net.ipv4.tcp_keepalive_intvl, value: "15" }

    # Buffer sizes
    - { name: net.core.rmem_max, value: "16777216" }
    - { name: net.core.wmem_max, value: "16777216" }
    - { name: net.ipv4.tcp_rmem, value: "4096 87380 16777216" }
    - { name: net.ipv4.tcp_wmem, value: "4096 65536 16777216" }
```

### Database Server Optimization

```yaml
- name: Configure kernel for PostgreSQL
  sysctl:
    name: "{{ item.name }}"
    value: "{{ item.value }}"
    sysctl_file: /etc/sysctl.d/50-postgresql.conf
  loop:
    - { name: vm.swappiness, value: "1" }
    - { name: vm.dirty_background_ratio, value: "5" }
    - { name: vm.dirty_ratio, value: "40" }
    - { name: vm.overcommit_memory, value: "2" }
    - { name: vm.overcommit_ratio, value: "80" }
    - { name: kernel.shmmax, value: "{{ ansible_memtotal_mb * 1024 * 1024 // 2 }}" }
    - { name: kernel.shmall, value: "{{ ansible_memtotal_mb * 256 }}" }
```

### Kubernetes Node Configuration

```yaml
- name: Configure kernel for Kubernetes
  sysctl:
    name: "{{ item.name }}"
    value: "{{ item.value }}"
    sysctl_file: /etc/sysctl.d/99-kubernetes.conf
  loop:
    - { name: net.bridge.bridge-nf-call-iptables, value: "1" }
    - { name: net.bridge.bridge-nf-call-ip6tables, value: "1" }
    - { name: net.ipv4.ip_forward, value: "1" }
    - { name: net.ipv4.conf.all.forwarding, value: "1" }
    - { name: fs.inotify.max_user_watches, value: "524288" }
    - { name: fs.inotify.max_user_instances, value: "512" }
```

### Security Hardening

```yaml
- name: Apply security hardening sysctl settings
  sysctl:
    name: "{{ item.name }}"
    value: "{{ item.value }}"
    sysctl_file: /etc/sysctl.d/80-security.conf
  loop:
    # Disable IP source routing
    - { name: net.ipv4.conf.all.accept_source_route, value: "0" }
    - { name: net.ipv4.conf.default.accept_source_route, value: "0" }

    # Disable ICMP redirect acceptance
    - { name: net.ipv4.conf.all.accept_redirects, value: "0" }
    - { name: net.ipv4.conf.default.accept_redirects, value: "0" }
    - { name: net.ipv4.conf.all.secure_redirects, value: "0" }

    # Enable SYN flood protection
    - { name: net.ipv4.tcp_syncookies, value: "1" }

    # Log martian packets
    - { name: net.ipv4.conf.all.log_martians, value: "1" }

    # Disable core dumps
    - { name: fs.suid_dumpable, value: "0" }

    # Randomize address space layout
    - { name: kernel.randomize_va_space, value: "2" }
```

## Notes

- Parameter names use either dots (net.ipv4.ip_forward) or slashes (net/ipv4/ip_forward)
- Values must be strings (even for numeric parameters)
- The module sets both runtime values and configuration files
- Changes to configuration files survive reboots
- Some parameters require a reboot to take full effect
- The sysctl_file directory is created if it does not exist

## Troubleshooting

### Parameter not found

Check if the parameter exists:

```bash
sysctl -a | grep parameter_name
ls /proc/sys/path/to/parameter
```

Some parameters only exist when certain kernel modules are loaded:

```bash
modprobe br_netfilter  # For bridge-nf-call-* parameters
```

### Value rejected

Some parameters have constraints. Check current value and valid range:

```bash
sysctl parameter_name
cat /proc/sys/path/to/parameter
```

### Changes not persisting

Verify the configuration file is being read:

```bash
sysctl --system
cat /etc/sysctl.d/*.conf
```

Check for conflicting files:

```bash
ls -la /etc/sysctl.d/
```

Later files (alphabetically) override earlier ones.

### Permission denied

Sysctl changes require root privileges. Ensure become is enabled:

```yaml
- name: Set sysctl parameter
  sysctl:
    name: net.ipv4.ip_forward
    value: "1"
  become: yes
```

### Network parameters not taking effect

Some network parameters require interface restart or reboot:

```bash
ip link set eth0 down && ip link set eth0 up
```

## See Also

- [service](service.md) - Restart services after kernel parameter changes
- [shell](shell.md) - Load kernel modules before setting parameters
- [command](command.md) - Verify kernel parameter values
