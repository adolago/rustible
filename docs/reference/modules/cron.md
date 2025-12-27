# cron - Manage Cron Jobs

## Synopsis

The `cron` module manages cron jobs for scheduled task execution. It supports user-specific crontabs, special time specifications (@reboot, @daily, etc.), environment variable management, and job disabling.

## Classification

**RemoteCommand** - This module executes crontab commands on remote hosts via SSH.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `name` | yes | - | string | Unique identifier for the cron job (used as comment marker). |
| `job` | conditional | - | string | Command to execute. Required when state is present. |
| `state` | no | present | string | Desired state: present, absent. |
| `user` | no | root | string | User whose crontab to modify. |
| `minute` | no | * | string | Minute field (0-59, *, */N, ranges, lists). |
| `hour` | no | * | string | Hour field (0-23, *, */N, ranges, lists). |
| `day` | no | * | string | Day of month field (1-31, *, */N, ranges, lists). |
| `month` | no | * | string | Month field (1-12, *, */N, ranges, lists). |
| `weekday` | no | * | string | Day of week field (0-7, *, */N, ranges, lists). 0 and 7 are Sunday. |
| `special_time` | no | - | string | Special time specification: @reboot, @yearly, @annually, @monthly, @weekly, @daily, @midnight, @hourly. |
| `disabled` | no | false | boolean | Comment out the job (disable without removing). |
| `cron_file` | no | - | string | Create job in /etc/cron.d/<name> instead of user crontab. |
| `env` | no | false | boolean | Whether this entry is an environment variable. |
| `backup` | no | false | boolean | Create backup before modifying crontab. |

## Cron Time Field Syntax

| Syntax | Description | Examples |
|--------|-------------|----------|
| `*` | Any value | Every minute, every hour, etc. |
| `N` | Specific value | `5` = at 5 |
| `N-M` | Range | `1-5` = 1 through 5 |
| `N,M,O` | List | `1,15,30` = at 1, 15, and 30 |
| `*/N` | Step | `*/5` = every 5 units |
| `N-M/S` | Range with step | `0-30/10` = 0, 10, 20, 30 |

## Special Time Values

| Value | Equivalent Schedule |
|-------|---------------------|
| `@reboot` | Run once at startup |
| `@yearly` / `@annually` | 0 0 1 1 * |
| `@monthly` | 0 0 1 * * |
| `@weekly` | 0 0 * * 0 |
| `@daily` / `@midnight` | 0 0 * * * |
| `@hourly` | 0 * * * * |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `name` | string | Job name |
| `job` | string | Command configured |
| `schedule` | string | Cron schedule |
| `user` | string | User for the crontab |
| `changed` | boolean | Whether changes were made |

## Examples

### Basic cron job

```yaml
- name: Run backup script daily at midnight
  cron:
    name: daily_backup
    job: /usr/local/bin/backup.sh
    hour: "0"
    minute: "0"
```

### Run every 5 minutes

```yaml
- name: Health check every 5 minutes
  cron:
    name: health_check
    job: /usr/local/bin/check_health.sh
    minute: "*/5"
```

### Run at specific times

```yaml
- name: Generate reports at 6 AM on weekdays
  cron:
    name: daily_report
    job: /usr/local/bin/generate_report.sh
    hour: "6"
    minute: "0"
    weekday: "1-5"
```

### Run on the first of each month

```yaml
- name: Monthly cleanup
  cron:
    name: monthly_cleanup
    job: /usr/local/bin/cleanup.sh
    day: "1"
    hour: "3"
    minute: "0"
```

### Use special time specification

```yaml
- name: Run at system startup
  cron:
    name: startup_script
    job: /usr/local/bin/on_boot.sh
    special_time: "@reboot"
```

### Run job as specific user

```yaml
- name: User-specific backup
  cron:
    name: user_backup
    job: /home/appuser/backup_data.sh
    user: appuser
    hour: "2"
    minute: "30"
```

### Disable a job temporarily

```yaml
- name: Disable maintenance job
  cron:
    name: maintenance
    job: /usr/local/bin/maintenance.sh
    hour: "4"
    minute: "0"
    disabled: yes
```

### Remove a cron job

```yaml
- name: Remove old job
  cron:
    name: old_backup
    state: absent
```

### Create system cron.d file

```yaml
- name: Create system cron job
  cron:
    name: logrotate
    job: /usr/sbin/logrotate /etc/logrotate.conf
    cron_file: logrotate
    hour: "3"
    minute: "0"
```

### Multiple schedules using list

```yaml
- name: Run at multiple times
  cron:
    name: multi_time_job
    job: /usr/local/bin/sync.sh
    hour: "6,12,18"
    minute: "0"
```

### Complex schedule

```yaml
- name: Run every 10 minutes during business hours
  cron:
    name: business_hours_sync
    job: /usr/local/bin/sync_crm.sh
    minute: "*/10"
    hour: "9-17"
    weekday: "1-5"
```

### Job with redirect

```yaml
- name: Cron with output redirect
  cron:
    name: quiet_job
    job: /usr/local/bin/noisy_script.sh > /dev/null 2>&1
    hour: "1"
    minute: "0"
```

### Job with environment handling

```yaml
- name: Job that sources environment
  cron:
    name: env_aware_job
    job: ". /home/app/.env && /home/app/run.sh"
    user: app
    hour: "*/2"
    minute: "0"
```

## Real-World Use Cases

### Database Backup Schedule

```yaml
- name: Full database backup weekly
  cron:
    name: db_full_backup
    job: /opt/backup/full_backup.sh >> /var/log/backup.log 2>&1
    weekday: "0"
    hour: "1"
    minute: "0"

- name: Incremental database backup daily
  cron:
    name: db_incremental_backup
    job: /opt/backup/incremental_backup.sh >> /var/log/backup.log 2>&1
    hour: "1"
    minute: "0"
    weekday: "1-6"
```

### Log Rotation and Cleanup

```yaml
- name: Rotate application logs
  cron:
    name: app_logrotate
    job: /usr/local/bin/rotate_logs.sh
    hour: "0"
    minute: "30"

- name: Clean old temp files
  cron:
    name: clean_temp
    job: find /tmp -type f -mtime +7 -delete
    hour: "4"
    minute: "0"
```

### Certificate Renewal

```yaml
- name: Check and renew SSL certificates
  cron:
    name: certbot_renew
    job: /usr/bin/certbot renew --quiet --post-hook "systemctl reload nginx"
    hour: "3"
    minute: "30"
    weekday: "1,4"
```

### Monitoring and Alerting

```yaml
- name: Disk space check
  cron:
    name: disk_space_alert
    job: /usr/local/bin/check_disk.sh | mail -s "Disk Alert" admin@example.com
    minute: "*/30"

- name: Service health check
  cron:
    name: service_monitor
    job: /usr/local/bin/check_services.sh
    minute: "*/5"
```

### Data Synchronization

```yaml
- name: Sync data to backup server
  cron:
    name: rsync_backup
    job: rsync -avz --delete /data/ backup@remote:/backup/data/
    hour: "*/4"
    minute: "0"
```

## Notes

- Jobs are identified by the `name` parameter using a RUSTIBLE marker comment
- Modifying a job with the same name will update the existing entry
- When using special_time, the individual time fields (minute, hour, etc.) are ignored
- The job command should include output redirection if you do not want email notifications
- For complex commands, consider using a wrapper script
- Disabled jobs are commented out with `#` prefix

## Troubleshooting

### Job not running

1. Check crontab syntax:
   ```bash
   crontab -l -u username
   crontab -e  # Opens editor for syntax check
   ```

2. Check cron daemon is running:
   ```bash
   systemctl status cron
   systemctl status crond
   ```

3. Check cron logs:
   ```bash
   grep CRON /var/log/syslog
   journalctl -u cron
   ```

### Permission denied

Ensure the script is executable:
```bash
chmod +x /path/to/script.sh
```

Check user permissions:
```bash
cat /etc/cron.allow
cat /etc/cron.deny
```

### Environment variables not available

Cron runs with a minimal environment. Set PATH and other variables in the script or use:

```yaml
- name: Job with explicit PATH
  cron:
    name: path_aware_job
    job: PATH=/usr/local/bin:/usr/bin:/bin /path/to/script.sh
    hour: "1"
    minute: "0"
```

### Job runs but command fails

Test the command as the cron user:
```bash
su - cronuser -c "/path/to/command"
```

Add logging to debug:
```yaml
- name: Debug job
  cron:
    name: debug_job
    job: "/path/to/script.sh >> /tmp/cron_debug.log 2>&1"
    minute: "*/5"
```

### Multiple jobs running simultaneously

Add random delay to prevent thundering herd:
```bash
sleep $((RANDOM % 60)) && /path/to/script.sh
```

## See Also

- [command](command.md) - Run one-time commands
- [shell](shell.md) - Run shell commands with complex syntax
- [service](service.md) - Manage cron daemon
- [file](file.md) - Manage script permissions
