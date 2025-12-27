# Stanley Deployment with Rustible

This directory contains Rustible playbooks and configurations for deploying the Stanley financial analysis platform.

## Overview

Rustible provides a high-performance, Ansible-compatible deployment solution for Stanley with:
- **Parallel execution** for faster deployments
- **Type safety** with compile-time validation
- **Async-first** architecture for optimal resource utilization
- **Native Rust modules** for core operations
- **Python fallback** for complex Ansible modules

## Quick Start

### Prerequisites

- Rustible installed on your control machine
- Target servers with SSH access
- Python 3.8+ on target servers
- Sufficient privileges for package installation

### 1. Configure Inventory

Edit `stanley-inventory.yml` to match your infrastructure:

```yaml
stanley_servers:
  hosts:
    your-stanley-server:
      ansible_host: 192.168.1.100
      ansible_user: deploy
      api_port: 8000
      gui_port: 3000
```

### 2. Set Environment Variables

Configure required API keys and secrets:

```bash
export OPENBB_API_KEY="your-openbb-api-key"
export SEC_IDENTITY="your-email@example.com"
export POSTGRES_PASSWORD="secure-database-password"
export REDIS_PASSWORD="secure-redis-password"
```

### 3. Deploy Stanley

Run the deployment playbook:

```bash
# Deploy to all servers
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml

# Deploy to specific environment
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml -l production

# Deploy with specific tags
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml --tags deploy,services

# Dry run
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml --check
```

## Directory Structure

```
deploy/
├── stanley-deployment.yml          # Main deployment playbook
├── stanley-inventory.yml           # Host inventory configuration
├── stanley/
│   └── templates/
│       ├── stanley.yaml.j2         # Stanley configuration template
│       ├── .env.j2                 # Environment variables template
│       ├── stanley-api.service.j2  # API systemd service
│       ├── stanley-gui.service.j2  # GUI systemd service
│       ├── backup.sh.j2            # Backup script
│       ├── maintenance.sh.j2       # Maintenance script
│       └── stanley-logrotate.j2    # Log rotation configuration
└── kubernetes/                     # Kubernetes manifests (optional)
```

## Playbook Structure

The main playbook (`stanley-deployment.yml`) consists of multiple plays:

### Play 1: Core Deployment
- System prerequisites installation
- Docker setup
- Stanley user creation
- Application deployment
- Database and Redis configuration
- Service management
- Health checks

### Play 2: Backup Configuration
- Backup script installation
- Automated backup scheduling
- Retention policy setup

### Play 3: Monitoring Setup
- Log rotation configuration
- System monitoring tools
- Health check automation

## Configuration Templates

### Stanley Configuration (`stanley.yaml.j2`)
- Server settings (host, port, workers)
- Database connection details
- Redis configuration
- API keys and secrets
- Feature flags
- Security settings
- Monitoring configuration

### Environment Variables (`.env.j2`)
- Application environment settings
- Database credentials
- API keys
- Path configurations
- Service ports

### Systemd Services
- **stanley-api.service**: FastAPI backend service
- **stanley-gui.service**: Rust GUI service
- Automatic restart on failure
- Resource limits and security hardening

## Deployment Tags

Use tags for selective deployment:

```bash
# Install packages only
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml --tags packages

# Configure services only
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml --tags services

# Deploy application only
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml --tags deploy

# Run health checks
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml --tags healthcheck
```

Available tags:
- `packages`: System package installation
- `docker`: Docker setup
- `user`: Stanley user creation
- `directories`: Directory structure setup
- `deploy`: Application deployment
- `database`: PostgreSQL setup
- `redis`: Redis setup
- `config`: Configuration files
- `services`: Systemd services
- `healthcheck`: Service health checks
- `backup`: Backup configuration
- `monitoring`: Monitoring setup

## Environment-Specific Deployments

### Production
```bash
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml -l production
```

### Development
```bash
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml -l development
```

### Staging
```bash
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml -l staging
```

## Maintenance Operations

### Manual Backup
```bash
# Run backup on all servers
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml --tags backup

# Run backup on specific server
rustible -i stanley-inventory.yml stanley_servers -m command -a "/usr/local/bin/stanley-backup"
```

### Maintenance Script
```bash
# Run maintenance tasks
rustible -i stanley-inventory.yml stanley_servers -m command -a "/usr/local/bin/stanley-maintenance"
```

### Service Management
```bash
# Restart services
rustible -i stanley-inventory.yml stanley_servers -m systemd -a "name=stanley-api state=restarted"
rustible -i stanley-inventory.yml stanley_servers -m systemd -a "name=stanley-gui state=restarted"

# Check service status
rustible -i stanley-inventory.yml stanley_servers -m systemd -a "name=stanley-api state=started"
```

## Security Considerations

### Encrypted Variables
Store sensitive data in encrypted files:

```bash
# Create encrypted vault file
rustible-vault create group_vars/all/vault.yml

# Edit vault file
rustible-vault edit group_vars/all/vault.yml
```

### Firewall Configuration
The playbook can configure UFW firewall:

```yaml
# In inventory
stanley_firewall_enabled: true
stanley_allowed_ports:
  - "8000"  # API
  - "3000"  # GUI
  - "22"    # SSH
```

### SSL/TLS Setup
For production deployments:

```yaml
# In inventory
stanley_ssl_enabled: true
stanley_domain: "stanley.yourcompany.com"
```

## Monitoring and Alerting

### Health Checks
The deployment includes automated health checks:
- API endpoint monitoring
- Database connectivity
- Redis availability
- Disk space monitoring

### Log Management
- Automatic log rotation
- Centralized logging setup
- Error tracking and alerting

### Metrics Collection
- Prometheus metrics endpoint
- System resource monitoring
- Application performance metrics

## Troubleshooting

### Common Issues

1. **Docker Installation Fails**
   ```bash
   # Check Docker repository configuration
   rustible -i stanley-inventory.yml stanley_servers -m apt -a "update_cache=yes"
   ```

2. **Database Connection Issues**
   ```bash
   # Check PostgreSQL status
   rustible -i stanley-inventory.yml stanley_servers -m systemd -a "name=postgresql state=started"
   ```

3. **Service Startup Failures**
   ```bash
   # Check service logs
   rustible -i stanley-inventory.yml stanley_servers -m command -a "journalctl -u stanley-api --no-pager -n 50"
   ```

### Debug Mode
Run with verbose output:
```bash
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml -vvv
```

### Check Mode
Preview changes without applying:
```bash
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml --check
```

## Performance Optimization

### Parallel Execution
Rustible executes tasks in parallel by default:
```bash
# Control parallelism
rustible-playbook -i stanley-inventory.yml stanley-deployment.yml -f 10
```

### Connection Pooling
Configure SSH connection pooling:
```bash
# In ansible.cfg
[ssh_connection]
ssh_args = -o ControlMaster=auto -o ControlPersist=60s
```

## Backup and Recovery

### Automated Backups
Backups run daily at 2 AM with:
- Database dumps
- Application data
- Configuration files
- Redis snapshots

### Manual Recovery
```bash
# Restore from backup
rustible -i stanley-inventory.yml stanley_servers -m command -a "pg_restore -d stanley /path/to/backup.sql"

# Restore application data
rustible -i stanley-inventory.yml stanley_servers -m unarchive -a "src=/path/to/data.tar.gz dest=/opt/stanley/"
```

## Migration from Ansible

Existing Ansible users can migrate seamlessly:

1. **Inventory Compatibility**: Use existing Ansible inventory files
2. **Playbook Syntax**: Identical YAML syntax
3. **Module Compatibility**: Most Ansible modules work unchanged
4. **Performance**: 2-4x faster execution with Rustible

### Migration Steps
1. Install Rustible
2. Test with existing playbooks
3. Gradually migrate to Rustible-specific optimizations
4. Update CI/CD pipelines

## Support

For issues and questions:
- Check the [Rustible documentation](https://github.com/ruvnet/rustible)
- Review deployment logs in `/var/log/stanley/`
- Check system logs with `journalctl -u stanley-api`
- Monitor service health at `http://your-server:8000/api/health`