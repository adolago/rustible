# hostname - Manage System Hostname

## Synopsis

The `hostname` module manages the system hostname, supporting both transient (runtime) and persistent hostname configuration. It automatically detects and uses the appropriate method (systemd hostnamectl or traditional /etc/hostname).

## Classification

**RemoteCommand** - This module executes hostname commands on remote hosts via SSH.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `name` | yes | - | string | Desired hostname (must be RFC 1123 compliant). |
| `use` | no | auto | string | Method to use: auto, systemd, file. |
| `pretty_hostname` | no | - | string | Human-readable hostname (systemd only). |
| `update_hosts` | no | true | boolean | Update /etc/hosts with the new hostname. |

## Use Values

| Value | Description |
|-------|-------------|
| `auto` | Automatically detect systemd or traditional method |
| `systemd` | Force use of hostnamectl |
| `file` | Force use of /etc/hostname |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `name` | string | The hostname that was set |
| `previous_name` | string | The hostname before the change |
| `strategy` | string | Method used (systemd or file) |
| `pretty_hostname` | string | Pretty hostname (if set) |
| `changed` | boolean | Whether changes were made |

## Examples

### Set simple hostname

```yaml
- name: Set hostname
  hostname:
    name: webserver01
```

### Set FQDN hostname

```yaml
- name: Set fully qualified hostname
  hostname:
    name: webserver01.example.com
```

### Set hostname with pretty name (systemd)

```yaml
- name: Set hostname with description
  hostname:
    name: db-primary-01
    pretty_hostname: "Primary Database Server"
```

### Set hostname without updating /etc/hosts

```yaml
- name: Set hostname only
  hostname:
    name: appserver01
    update_hosts: no
```

### Force systemd method

```yaml
- name: Set hostname via hostnamectl
  hostname:
    name: server01
    use: systemd
```

### Force traditional file method

```yaml
- name: Set hostname via /etc/hostname
  hostname:
    name: server01
    use: file
```

### Set hostname based on inventory

```yaml
- name: Set hostname from inventory name
  hostname:
    name: "{{ inventory_hostname }}"
```

### Set hostname with domain from variable

```yaml
- name: Set hostname with domain
  hostname:
    name: "{{ inventory_hostname_short }}.{{ domain }}"
```

## Real-World Use Cases

### Cloud Instance Provisioning

```yaml
- name: Set hostname for cloud instance
  hostname:
    name: "{{ cloud_instance_id }}-{{ environment }}"

- name: Update hostname in cloud metadata
  command: cloud-init single --name hostname
  when: ansible_virtualization_type == "kvm"
```

### Multi-tier Application Deployment

```yaml
- name: Set hostname based on role
  hostname:
    name: "{{ service_role }}-{{ ansible_host | regex_replace('\\..*', '') }}.{{ domain }}"
    pretty_hostname: "{{ service_role | capitalize }} Server"
```

### Container Host Naming

```yaml
- name: Configure Docker host hostname
  hostname:
    name: "docker-{{ region }}-{{ availability_zone }}-{{ sequence_number }}"
    pretty_hostname: "Docker Host in {{ region | upper }}"
```

### Kubernetes Node Naming

```yaml
- name: Set Kubernetes node hostname
  hostname:
    name: "k8s-{{ node_role }}-{{ node_index }}.{{ cluster_domain }}"
```

## Hostname Validation

Hostnames must comply with RFC 1123:

- Maximum 253 characters total
- Each label (between dots) maximum 63 characters
- Start and end with alphanumeric characters
- Contain only alphanumeric characters and hyphens
- No consecutive dots

### Valid Hostnames

- `server1`
- `web-server`
- `db01.example.com`
- `mail-server-01.corp.example.com`

### Invalid Hostnames

- `-server` (starts with hyphen)
- `server-` (ends with hyphen)
- `server_name` (contains underscore)
- `server name` (contains space)
- `.server` (starts with dot)

## Notes

- The module automatically detects systemd systems and uses hostnamectl
- On non-systemd systems, it updates /etc/hostname and runs the hostname command
- When update_hosts is true, it replaces the old hostname in /etc/hosts
- Pretty hostname is only supported on systemd-based systems
- Changes are applied immediately (no reboot required)
- The module validates hostname format before making changes

## Troubleshooting

### Hostname reverts after reboot

Check if cloud-init or another tool is overwriting the hostname:

```bash
# Check cloud-init configuration
cat /etc/cloud/cloud.cfg | grep -A5 preserve_hostname

# Preserve hostname in cloud-init
echo "preserve_hostname: true" >> /etc/cloud/cloud.cfg.d/99-hostname.cfg
```

### hostnamectl fails

Ensure systemd is running:

```bash
systemctl status systemd-hostnamed
systemctl restart systemd-hostnamed
```

### /etc/hosts not updated correctly

The module only replaces exact matches of the old hostname. For complex /etc/hosts configurations, manage them separately with the lineinfile module.

### Hostname change not reflected in prompt

Log out and log back in, or run:

```bash
exec bash
```

### DNS resolution issues after hostname change

Update your DNS server or check /etc/resolv.conf:

```bash
# Verify hostname resolution
hostname -f
getent hosts $(hostname)
```

## See Also

- [lineinfile](lineinfile.md) - Manage /etc/hosts entries
- [file](file.md) - Manage hostname-related files
- [service](service.md) - Restart services after hostname change
