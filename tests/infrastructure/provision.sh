#!/bin/bash
# Rustible Test Infrastructure Provisioning Script
# Deploys test VMs and containers to svr-host (Proxmox VE)

set -euo pipefail

# Configuration
PROXMOX_HOST="${PROXMOX_HOST:-svr-host}"
PROXMOX_USER="${PROXMOX_USER:-artur}"
STORAGE="${STORAGE:-local-lvm}"
TEMPLATE="${TEMPLATE:-local:vztmpl/ubuntu-24.04-standard_24.04-2_amd64.tar.zst}"
BRIDGE="${BRIDGE:-vmbr0}"
SSH_KEY_FILE="${SSH_KEY_FILE:-$HOME/.ssh/id_ed25519.pub}"
TEST_USER="testuser"
TEST_PASSWORD="rustible-test-2024"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# SSH wrapper for Proxmox commands
pve_cmd() {
    ssh "${PROXMOX_USER}@${PROXMOX_HOST}" "$@"
}

# Check if container/VM exists
exists() {
    local vmid=$1
    pve_cmd "pct status $vmid 2>/dev/null || qm status $vmid 2>/dev/null" &>/dev/null
}

# Create LXC container
create_lxc() {
    local vmid=$1
    local hostname=$2
    local cores=$3
    local memory=$4
    local disk=$5
    local ip=$6

    if exists "$vmid"; then
        log_warn "Container $vmid ($hostname) already exists, skipping"
        return 0
    fi

    log_info "Creating LXC container $vmid ($hostname)..."

    pve_cmd "pct create $vmid $TEMPLATE \
        --hostname $hostname \
        --cores $cores \
        --memory $memory \
        --rootfs ${STORAGE}:$disk \
        --net0 name=eth0,bridge=${BRIDGE},ip=${ip}/24,gw=192.168.178.1 \
        --nameserver 192.168.178.102 \
        --features nesting=1 \
        --unprivileged 1 \
        --onboot 0 \
        --start 0"

    log_success "Container $vmid created"
}

# Create QEMU VM from cloud image
create_vm() {
    local vmid=$1
    local hostname=$2
    local cores=$3
    local memory=$4
    local disk=$5
    local ip=$6
    local iso=$7

    if exists "$vmid"; then
        log_warn "VM $vmid ($hostname) already exists, skipping"
        return 0
    fi

    log_info "Creating VM $vmid ($hostname)..."

    pve_cmd "qm create $vmid \
        --name $hostname \
        --cores $cores \
        --memory $memory \
        --net0 virtio,bridge=${BRIDGE} \
        --scsihw virtio-scsi-pci \
        --scsi0 ${STORAGE}:$disk \
        --ide2 local:iso/${iso},media=cdrom \
        --boot order=scsi0;ide2 \
        --ostype l26 \
        --agent 1 \
        --onboot 0"

    log_success "VM $vmid created"
}

# Start container/VM and wait for it
start_and_wait() {
    local vmid=$1
    local type=$2  # lxc or vm
    local max_wait=120

    log_info "Starting $type $vmid..."

    if [ "$type" = "lxc" ]; then
        pve_cmd "pct start $vmid" 2>/dev/null || true

        # Wait for container to be running
        for i in $(seq 1 $max_wait); do
            if pve_cmd "pct status $vmid" 2>/dev/null | grep -q "running"; then
                log_success "$type $vmid is running"
                sleep 5  # Extra time for services to start
                return 0
            fi
            sleep 1
        done
    else
        pve_cmd "qm start $vmid" 2>/dev/null || true

        for i in $(seq 1 $max_wait); do
            if pve_cmd "qm status $vmid" 2>/dev/null | grep -q "running"; then
                log_success "$type $vmid is running"
                sleep 10  # VMs need more time to boot
                return 0
            fi
            sleep 1
        done
    fi

    log_error "Timeout waiting for $type $vmid to start"
    return 1
}

# Configure SSH in container
configure_lxc_ssh() {
    local vmid=$1
    local ip=$2

    log_info "Configuring SSH for container $vmid..."

    # Install SSH and create test user
    pve_cmd "pct exec $vmid -- bash -c '
        apt-get update -qq
        apt-get install -y -qq openssh-server sudo python3 >/dev/null 2>&1

        # Create test user
        useradd -m -s /bin/bash $TEST_USER 2>/dev/null || true
        echo \"$TEST_USER:$TEST_PASSWORD\" | chpasswd
        echo \"$TEST_USER ALL=(ALL) NOPASSWD:ALL\" > /etc/sudoers.d/$TEST_USER

        # Configure SSH
        mkdir -p /home/$TEST_USER/.ssh
        chmod 700 /home/$TEST_USER/.ssh
        chown -R $TEST_USER:$TEST_USER /home/$TEST_USER/.ssh

        # Enable SSH
        systemctl enable ssh
        systemctl start ssh
    '"

    # Copy SSH key if provided
    if [ -f "$SSH_KEY_FILE" ]; then
        local pubkey=$(cat "$SSH_KEY_FILE")
        pve_cmd "pct exec $vmid -- bash -c 'echo \"$pubkey\" >> /home/$TEST_USER/.ssh/authorized_keys && chmod 600 /home/$TEST_USER/.ssh/authorized_keys && chown $TEST_USER:$TEST_USER /home/$TEST_USER/.ssh/authorized_keys'"
    fi

    log_success "SSH configured for container $vmid"
}

# Configure Docker host
configure_docker_host() {
    local vmid=$1

    log_info "Configuring Docker on VM $vmid..."

    pve_cmd "qm guest exec $vmid -- bash -c '
        # Install Docker
        curl -fsSL https://get.docker.com | sh

        # Add test user to docker group
        usermod -aG docker $TEST_USER 2>/dev/null || true

        # Enable Docker API on TCP (for testing)
        mkdir -p /etc/systemd/system/docker.service.d
        cat > /etc/systemd/system/docker.service.d/override.conf << EOF
[Service]
ExecStart=
ExecStart=/usr/bin/dockerd -H fd:// -H tcp://0.0.0.0:2375 --containerd=/run/containerd/containerd.sock
EOF

        systemctl daemon-reload
        systemctl restart docker
    '"

    log_success "Docker configured on VM $vmid"
}

# Deploy all infrastructure
deploy() {
    log_info "Starting test infrastructure deployment to $PROXMOX_HOST..."

    # Ensure template is available
    log_info "Checking for LXC template..."
    if ! pve_cmd "ls /var/lib/vz/template/cache/ubuntu-24.04*.tar.zst 2>/dev/null" | grep -q ubuntu; then
        log_info "Downloading Ubuntu 24.04 LXC template..."
        pve_cmd "pveam download local ubuntu-24.04-standard_24.04-2_amd64.tar.zst"
    fi

    # Create test controller
    log_info "=== Creating Test Controller ==="
    create_lxc 301 "test-ctrl" 4 8192 32 "192.168.178.201"
    start_and_wait 301 lxc
    configure_lxc_ssh 301 "192.168.178.201"

    # Install Rust toolchain on test controller
    log_info "Installing Rust toolchain on test controller..."
    pve_cmd "pct exec 301 -- bash -c '
        apt-get install -y -qq build-essential pkg-config libssl-dev curl git >/dev/null 2>&1
        su - $TEST_USER -c \"curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y\"
    '"

    # Create SSH target fleet
    log_info "=== Creating SSH Target Fleet ==="
    for i in $(seq 1 5); do
        vmid=$((400 + i))
        ip="192.168.178.$((140 + i))"
        create_lxc $vmid "ssh-target-$vmid" 2 2048 8 "$ip"
        start_and_wait $vmid lxc
        configure_lxc_ssh $vmid "$ip"
    done

    # Create scale test fleet
    log_info "=== Creating Scale Test Fleet ==="
    for i in $(seq 1 10); do
        vmid=$((500 + i))
        ip="192.168.178.$((150 + i))"
        create_lxc $vmid "scale-$vmid" 1 1024 4 "$ip"
        start_and_wait $vmid lxc
        configure_lxc_ssh $vmid "$ip"
    done

    # Generate test inventory
    log_info "=== Generating Test Inventory ==="
    generate_inventory

    # Generate SSH config
    log_info "=== Generating SSH Config ==="
    generate_ssh_config

    log_success "Test infrastructure deployment complete!"
    log_info "Test inventory written to: tests/infrastructure/test_inventory.yml"
    log_info "SSH config written to: tests/infrastructure/ssh_config"
}

# Generate test inventory file
generate_inventory() {
    cat > "$(dirname "$0")/test_inventory.yml" << 'EOF'
---
all:
  vars:
    ansible_user: testuser
    ansible_ssh_private_key_file: "{{ lookup('env', 'HOME') }}/.ssh/id_ed25519"
    ansible_python_interpreter: /usr/bin/python3
  children:
    test_controller:
      hosts:
        test-ctrl:
          ansible_host: 192.168.178.201
    ssh_targets:
      hosts:
        ssh-target-401:
          ansible_host: 192.168.178.141
        ssh-target-402:
          ansible_host: 192.168.178.142
        ssh-target-403:
          ansible_host: 192.168.178.143
        ssh-target-404:
          ansible_host: 192.168.178.144
        ssh-target-405:
          ansible_host: 192.168.178.145
    scale_fleet:
      hosts:
        scale-501:
          ansible_host: 192.168.178.151
        scale-502:
          ansible_host: 192.168.178.152
        scale-503:
          ansible_host: 192.168.178.153
        scale-504:
          ansible_host: 192.168.178.154
        scale-505:
          ansible_host: 192.168.178.155
        scale-506:
          ansible_host: 192.168.178.156
        scale-507:
          ansible_host: 192.168.178.157
        scale-508:
          ansible_host: 192.168.178.158
        scale-509:
          ansible_host: 192.168.178.159
        scale-510:
          ansible_host: 192.168.178.160
    docker_hosts:
      hosts:
        docker-host:
          ansible_host: 192.168.178.210
          docker_api: "tcp://192.168.178.210:2375"
EOF
    log_success "Test inventory generated"
}

# Generate SSH config for easy access
generate_ssh_config() {
    cat > "$(dirname "$0")/ssh_config" << 'EOF'
# Rustible Test Infrastructure SSH Config
# Usage: ssh -F tests/infrastructure/ssh_config <hostname>

Host test-ctrl
    HostName 192.168.178.201
    User testuser

Host ssh-target-*
    User testuser

Host ssh-target-401
    HostName 192.168.178.141

Host ssh-target-402
    HostName 192.168.178.142

Host ssh-target-403
    HostName 192.168.178.143

Host ssh-target-404
    HostName 192.168.178.144

Host ssh-target-405
    HostName 192.168.178.145

Host scale-*
    User testuser

Host scale-501
    HostName 192.168.178.151

Host scale-502
    HostName 192.168.178.152

Host scale-503
    HostName 192.168.178.153

Host scale-504
    HostName 192.168.178.154

Host scale-505
    HostName 192.168.178.155

Host scale-506
    HostName 192.168.178.156

Host scale-507
    HostName 192.168.178.157

Host scale-508
    HostName 192.168.178.158

Host scale-509
    HostName 192.168.178.159

Host scale-510
    HostName 192.168.178.160

Host docker-host
    HostName 192.168.178.210
    User testuser

Host *
    IdentityFile ~/.ssh/id_ed25519
    StrictHostKeyChecking no
    UserKnownHostsFile /dev/null
    LogLevel ERROR
EOF
    log_success "SSH config generated"
}

# Start all test infrastructure
start() {
    log_info "Starting all test infrastructure..."

    # Start containers
    for vmid in 301 401 402 403 404 405 501 502 503 504 505 506 507 508 509 510; do
        if exists $vmid; then
            pve_cmd "pct start $vmid 2>/dev/null" || true
            log_info "Started container $vmid"
        fi
    done

    # Start VMs
    for vmid in 310 320 321 322; do
        if exists $vmid; then
            pve_cmd "qm start $vmid 2>/dev/null" || true
            log_info "Started VM $vmid"
        fi
    done

    log_success "All test infrastructure started"
}

# Stop all test infrastructure
stop() {
    log_info "Stopping all test infrastructure..."

    # Stop containers
    for vmid in 301 401 402 403 404 405 501 502 503 504 505 506 507 508 509 510; do
        if exists $vmid; then
            pve_cmd "pct stop $vmid 2>/dev/null" || true
            log_info "Stopped container $vmid"
        fi
    done

    # Stop VMs
    for vmid in 310 320 321 322; do
        if exists $vmid; then
            pve_cmd "qm stop $vmid 2>/dev/null" || true
            log_info "Stopped VM $vmid"
        fi
    done

    log_success "All test infrastructure stopped"
}

# Teardown (destroy scale fleet, keep core)
teardown() {
    log_info "Tearing down scale test fleet..."

    for vmid in $(seq 501 510); do
        if exists $vmid; then
            pve_cmd "pct stop $vmid 2>/dev/null" || true
            pve_cmd "pct destroy $vmid 2>/dev/null" || true
            log_info "Destroyed container $vmid"
        fi
    done

    log_success "Scale fleet destroyed"
}

# Destroy all test infrastructure
destroy() {
    log_warn "This will destroy ALL test infrastructure. Are you sure? (y/N)"
    read -r confirm
    if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
        log_info "Aborted"
        return 0
    fi

    log_info "Destroying all test infrastructure..."

    # Destroy containers
    for vmid in 301 401 402 403 404 405 501 502 503 504 505 506 507 508 509 510; do
        if exists $vmid; then
            pve_cmd "pct stop $vmid 2>/dev/null" || true
            pve_cmd "pct destroy $vmid 2>/dev/null" || true
            log_info "Destroyed container $vmid"
        fi
    done

    # Destroy VMs
    for vmid in 310 320 321 322; do
        if exists $vmid; then
            pve_cmd "qm stop $vmid 2>/dev/null" || true
            pve_cmd "qm destroy $vmid 2>/dev/null" || true
            log_info "Destroyed VM $vmid"
        fi
    done

    log_success "All test infrastructure destroyed"
}

# Create snapshot
snapshot() {
    local name="${1:-$(date +%Y%m%d-%H%M%S)}"
    log_info "Creating snapshot: $name"

    for vmid in 301 401 402 403 404 405; do
        if exists $vmid; then
            pve_cmd "pct snapshot $vmid $name" 2>/dev/null || true
            log_info "Snapshot $name created for container $vmid"
        fi
    done

    log_success "Snapshots created"
}

# Rollback to snapshot
rollback() {
    local name=$1
    if [ -z "$name" ]; then
        log_error "Usage: $0 rollback <snapshot-name>"
        return 1
    fi

    log_info "Rolling back to snapshot: $name"

    for vmid in 301 401 402 403 404 405; do
        if exists $vmid; then
            pve_cmd "pct rollback $vmid $name" 2>/dev/null || true
            log_info "Rolled back container $vmid to $name"
        fi
    done

    log_success "Rollback complete"
}

# View logs
logs() {
    local vmid=$1
    if [ -z "$vmid" ]; then
        log_error "Usage: $0 logs <vmid>"
        return 1
    fi

    pve_cmd "pct exec $vmid -- journalctl -n 100 --no-pager" 2>/dev/null || \
    pve_cmd "qm guest exec $vmid -- journalctl -n 100 --no-pager" 2>/dev/null
}

# Status check
status() {
    log_info "Test Infrastructure Status:"
    echo
    echo "=== LXC Containers ==="
    pve_cmd "pct list" 2>/dev/null | grep -E "^(VMID|30|40|50)" || echo "No containers found"
    echo
    echo "=== QEMU VMs ==="
    pve_cmd "qm list" 2>/dev/null | grep -E "^(VMID|31|32)" || echo "No VMs found"
}

# Main
case "${1:-help}" in
    deploy)
        deploy
        ;;
    start)
        start
        ;;
    stop)
        stop
        ;;
    teardown)
        teardown
        ;;
    destroy)
        destroy
        ;;
    snapshot)
        snapshot "${2:-}"
        ;;
    rollback)
        rollback "${2:-}"
        ;;
    logs)
        logs "${2:-}"
        ;;
    status)
        status
        ;;
    inventory)
        generate_inventory
        ;;
    ssh-config)
        generate_ssh_config
        ;;
    help|*)
        echo "Rustible Test Infrastructure Provisioning"
        echo
        echo "Usage: $0 <command> [args]"
        echo
        echo "Commands:"
        echo "  deploy      - Deploy all test infrastructure"
        echo "  start       - Start all test VMs/containers"
        echo "  stop        - Stop all test VMs/containers"
        echo "  teardown    - Destroy scale fleet, keep core"
        echo "  destroy     - Destroy ALL test infrastructure"
        echo "  snapshot    - Create snapshot of all containers"
        echo "  rollback    - Rollback to named snapshot"
        echo "  logs        - View logs for a container/VM"
        echo "  status      - Show infrastructure status"
        echo "  inventory   - Regenerate test inventory"
        echo "  ssh-config  - Regenerate SSH config"
        echo
        echo "Environment:"
        echo "  PROXMOX_HOST  - Proxmox host (default: svr-host)"
        echo "  PROXMOX_USER  - SSH user (default: artur)"
        echo "  SSH_KEY_FILE  - SSH public key to deploy"
        ;;
esac
