#!/bin/bash
# Entrypoint script for web server test container

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

# Create fake service status files for testing
mkdir -p /var/run
touch /var/run/nginx.pid 2>/dev/null || true

# Create a fake nginx binary for testing
cat > /usr/local/bin/nginx-status << 'EOF'
#!/bin/bash
echo "nginx is running"
exit 0
EOF
chmod +x /usr/local/bin/nginx-status

# Execute the main command
exec "$@"
