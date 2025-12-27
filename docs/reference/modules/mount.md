# mount - Manage Filesystem Mount Points

## Synopsis

The `mount` module manages filesystem mount points, including mounting, unmounting, and configuring persistent mounts in /etc/fstab. It supports all common mount options and provides idempotent operations for system administration.

## Classification

**RemoteCommand** - This module executes mount commands on remote hosts via SSH.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `path` | yes | - | string | Mount point path (directory where filesystem will be mounted). |
| `src` | conditional | - | string | Device, partition, NFS path, or UUID to mount. Required when state is mounted or present. |
| `fstype` | no | auto | string | Filesystem type (ext4, xfs, nfs, tmpfs, etc.). |
| `opts` | no | defaults | string | Mount options (comma-separated, e.g., "rw,noatime,nodiratime"). |
| `state` | no | mounted | string | Desired state: mounted, unmounted, present, absent, remounted. |
| `dump` | no | 0 | integer | Dump frequency for fstab (0 or 1). |
| `passno` | no | 0 | integer | Pass number for fsck (0, 1, or 2). |
| `fstab` | no | /etc/fstab | string | Path to fstab file to manage. |
| `force` | no | false | boolean | Force unmount even if filesystem is busy. |

## State Values

| State | Description |
|-------|-------------|
| `mounted` | Mount the filesystem and add entry to fstab |
| `unmounted` | Unmount the filesystem but keep fstab entry |
| `present` | Add entry to fstab without mounting |
| `absent` | Unmount and remove from fstab |
| `remounted` | Remount with updated options (must already be mounted) |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `path` | string | Mount point path |
| `src` | string | Source device or path |
| `fstype` | string | Filesystem type |
| `opts` | string | Mount options |
| `changed` | boolean | Whether changes were made |
| `msg` | string | Status message |

## Examples

### Mount an ext4 partition

```yaml
- name: Mount data partition
  mount:
    path: /data
    src: /dev/sdb1
    fstype: ext4
    state: mounted
```

### Mount with performance options

```yaml
- name: Mount SSD with optimized options
  mount:
    path: /var/lib/mysql
    src: /dev/nvme0n1p1
    fstype: ext4
    opts: rw,noatime,nodiratime,barrier=0
    state: mounted
```

### Mount by UUID (recommended for stability)

```yaml
- name: Mount by UUID
  mount:
    path: /backup
    src: UUID=a1b2c3d4-e5f6-7890-abcd-ef1234567890
    fstype: xfs
    state: mounted
```

### Mount NFS share

```yaml
- name: Mount NFS home directories
  mount:
    path: /home
    src: nfs-server:/exports/home
    fstype: nfs
    opts: rw,hard,intr,rsize=8192,wsize=8192
    state: mounted
```

### Mount NFS with autofs-style timeout

```yaml
- name: Mount NFS with soft timeout
  mount:
    path: /mnt/shared
    src: 192.168.1.10:/data
    fstype: nfs4
    opts: soft,timeo=100,retrans=3,_netdev
    state: mounted
```

### Mount tmpfs for temporary storage

```yaml
- name: Create tmpfs for application cache
  mount:
    path: /var/cache/myapp
    src: tmpfs
    fstype: tmpfs
    opts: size=512M,mode=0755
    state: mounted
```

### Mount with specific user/group access

```yaml
- name: Mount USB drive for specific user
  mount:
    path: /mnt/usb
    src: /dev/sdc1
    fstype: vfat
    opts: uid=1000,gid=1000,umask=022
    state: mounted
```

### Add to fstab without mounting

```yaml
- name: Configure mount for next boot
  mount:
    path: /mnt/backup
    src: /dev/sdd1
    fstype: ext4
    opts: defaults,noauto
    state: present
```

### Unmount but keep fstab entry

```yaml
- name: Temporarily unmount for maintenance
  mount:
    path: /mnt/data
    state: unmounted
```

### Remove mount completely

```yaml
- name: Remove obsolete mount point
  mount:
    path: /mnt/old-storage
    state: absent
```

### Force unmount busy filesystem

```yaml
- name: Force unmount
  mount:
    path: /mnt/stuck
    state: absent
    force: yes
```

### Remount with new options

```yaml
- name: Remount read-only for backup
  mount:
    path: /data
    opts: ro,noatime
    state: remounted
```

### Mount bind directory

```yaml
- name: Create bind mount
  mount:
    path: /var/www/html/static
    src: /opt/static-content
    fstype: none
    opts: bind
    state: mounted
```

### Complete storage setup example

```yaml
- name: Create mount point directory
  file:
    path: /data
    state: directory
    mode: '0755'

- name: Mount data filesystem
  mount:
    path: /data
    src: /dev/mapper/vg_data-lv_data
    fstype: xfs
    opts: defaults,noatime
    dump: 1
    passno: 2
    state: mounted

- name: Set ownership after mounting
  file:
    path: /data
    owner: appuser
    group: appgroup
    mode: '0755'
```

### CIFS/SMB share mount

```yaml
- name: Mount Windows share
  mount:
    path: /mnt/windows
    src: //server/share
    fstype: cifs
    opts: username=user,password=pass,uid=1000,gid=1000
    state: mounted
```

## Real-World Use Cases

### Database Server Data Volume

```yaml
- name: Configure PostgreSQL data volume
  mount:
    path: /var/lib/postgresql/data
    src: /dev/mapper/vg_pg-lv_data
    fstype: xfs
    opts: noatime,nodiratime,nobarrier
    dump: 1
    passno: 2
    state: mounted
  notify: Restart PostgreSQL
```

### Docker Data Directory

```yaml
- name: Mount dedicated Docker storage
  mount:
    path: /var/lib/docker
    src: /dev/sdb1
    fstype: xfs
    opts: defaults,pquota
    state: mounted
  notify: Restart Docker
```

### Kubernetes Node Storage

```yaml
- name: Mount ephemeral storage for pods
  mount:
    path: /var/lib/kubelet/pods
    src: /dev/nvme1n1
    fstype: ext4
    opts: defaults,noatime
    state: mounted
```

## Notes

- The module creates mount point directories if they do not exist
- Always use UUIDs or labels instead of device names for production systems
- NFS mounts should include `_netdev` option to ensure network availability
- The fstab file is backed up before modifications
- Remount state requires the filesystem to already be mounted
- Force unmount should be used with caution on busy filesystems

## Troubleshooting

### Mount fails with "device or resource busy"

The filesystem is in use. Check for open files:

```bash
lsof +D /mount/point
fuser -v /mount/point
```

Use `force: yes` only as a last resort.

### NFS mount hangs

Check network connectivity and NFS server status:

```bash
showmount -e nfs-server
rpcinfo -p nfs-server
```

Use `soft` mount option for non-critical mounts.

### Permission denied after mount

For VFAT/NTFS/CIFS, use uid/gid/umask options. For others, check SELinux contexts:

```bash
restorecon -Rv /mount/point
```

### Mount not persisting after reboot

Verify fstab entry is correct:

```bash
mount -a
cat /etc/fstab
```

Check that `noauto` option is not set if automatic mounting is desired.

### UUID not found

Find the correct UUID:

```bash
blkid /dev/sdb1
lsblk -f
```

## See Also

- [file](file.md) - Create mount point directories
- [copy](copy.md) - Copy files to mounted filesystems
- [service](service.md) - Manage services dependent on mounts
