#!/bin/bash
# Entrypoint script for database test container

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
mkdir -p /var/run/postgresql
touch /var/run/postgresql/.s.PGSQL.5432 2>/dev/null || true

# Create a fake pg_isready binary for testing
cat > /usr/local/bin/pg_isready << 'EOF'
#!/bin/bash
echo "accepting connections"
exit 0
EOF
chmod +x /usr/local/bin/pg_isready

# Execute the main command
exec "$@"
