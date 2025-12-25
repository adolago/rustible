#!/bin/bash
# Entrypoint script for application server test container

# Create SSH host keys if they don't exist
if [ ! -f /etc/ssh/ssh_host_rsa_key ]; then
    ssh-keygen -A
fi

# Add authorized keys if provided via environment
if [ -n "$SSH_AUTHORIZED_KEY" ]; then
    echo "$SSH_AUTHORIZED_KEY" > /home/testuser/.ssh/authorized_keys
    chmod 600 /home/testuser/.ssh/authorized_keys
    chown testuser:testuser /home/testuser/.ssh/authorized_keys
fi

# Create application directories
mkdir -p /opt/apps/current
mkdir -p /opt/apps/releases
mkdir -p /opt/apps/shared/log
mkdir -p /opt/apps/shared/config
chown -R appuser:appuser /opt/apps

# Execute the main command
exec "$@"
