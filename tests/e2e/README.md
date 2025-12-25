# Rustible E2E Tests

This directory contains end-to-end (E2E) tests that validate Rustible's functionality against real Docker containers. These tests execute complete playbook scenarios to ensure Rustible works correctly in realistic deployment situations.

## Overview

The E2E test suite covers five major deployment scenarios:

| Scenario | Description | Hosts |
|----------|-------------|-------|
| Web Server Setup | nginx configuration, virtual hosts, static content | web1, web2 |
| Database Configuration | PostgreSQL setup, pg_hba.conf, backup scripts | db1 |
| User Management | System users, SSH keys, sudo configuration | all |
| Application Deployment | Capistrano-style releases, symlinks, config files | app1, app2 |
| Multi-Tier Application | Complete stack orchestration across all tiers | all |

## Quick Start

### Prerequisites

- Docker and Docker Compose (v2.0+)
- Rust toolchain (stable)
- netcat (`nc`) for connectivity checks

### Running the Tests

```bash
# Navigate to the e2e directory
cd tests/e2e

# Option 1: Use the helper script (recommended)
chmod +x scripts/run-e2e-tests.sh
./scripts/run-e2e-tests.sh setup    # Build and start containers
./scripts/run-e2e-tests.sh test     # Run all E2E tests
./scripts/run-e2e-tests.sh cleanup  # Clean up

# Option 2: Manual approach
cd docker
docker compose up -d --build

# Wait for containers to be ready
sleep 10

# Run tests
cd ../..
RUSTIBLE_E2E_DOCKER_ENABLED=1 cargo test --test e2e_docker_tests -- --nocapture

# Clean up
cd tests/e2e/docker
docker compose down -v
```

### Running Specific Tests

```bash
# Run only webserver tests
./scripts/run-e2e-tests.sh test webserver

# Run with verbose output
RUSTIBLE_E2E_VERBOSE=1 ./scripts/run-e2e-tests.sh test

# Run idempotency tests
./scripts/run-e2e-tests.sh test idempotency
```

## Directory Structure

```
tests/e2e/
|-- docker/
|   |-- docker-compose.yml      # Container orchestration
|   |-- Dockerfile.webserver    # Web server container
|   |-- Dockerfile.database     # Database container
|   |-- Dockerfile.appserver    # Application server container
|   |-- entrypoint-*.sh         # Container entrypoint scripts
|
|-- playbooks/
|   |-- 01_webserver_setup.yml  # nginx installation and configuration
|   |-- 02_database_setup.yml   # PostgreSQL configuration
|   |-- 03_user_management.yml  # User and access management
|   |-- 04_app_deployment.yml   # Application deployment workflow
|   |-- 05_multi_tier_app.yml   # Full stack orchestration
|   |-- inventory.yml           # Test inventory
|
|-- templates/
|   |-- nginx.conf.j2           # nginx main configuration
|   |-- default-site.conf.j2    # nginx default site
|   |-- postgresql.conf.j2      # PostgreSQL configuration
|   |-- pg_hba.conf.j2          # PostgreSQL authentication
|   |-- app_config.yml.j2       # Application configuration
|   |-- ssh_config.j2           # SSH client configuration
|   |-- upstream.conf.j2        # nginx upstream servers
|
|-- files/                      # Static files for deployment
|-- scripts/
|   |-- run-e2e-tests.sh        # Test runner helper script
|
|-- README.md                   # This file
```

## Docker Environment

### Container Network

All containers connect to a custom bridge network (`rustible_e2e`) with the following IP assignments:

| Container | IP Address | SSH Port | Other Ports |
|-----------|------------|----------|-------------|
| web1 | 172.28.1.10 | 2221 | 8081 (HTTP) |
| web2 | 172.28.1.11 | 2222 | 8082 (HTTP) |
| db1 | 172.28.2.10 | 2223 | 5433 (PostgreSQL) |
| app1 | 172.28.3.10 | 2224 | 8083 (App) |
| app2 | 172.28.3.11 | 2225 | 8084 (App) |

### Container Credentials

- **SSH User**: testuser
- **SSH Password**: testpassword
- **Sudo**: Passwordless for testuser

### Container Features

Each container includes:
- OpenSSH server for remote access
- Python 3 for module execution
- sudo for privilege escalation
- Pre-created directories for testing

## Test Scenarios

### 1. Web Server Setup (`01_webserver_setup.yml`)

Tests the following modules and features:
- `file`: Directory creation with permissions
- `template`: nginx.conf and site configuration
- `copy`: Static content deployment
- `command`: Configuration verification
- `lineinfile`: Configuration modifications
- Handlers: nginx reload on configuration change

### 2. Database Configuration (`02_database_setup.yml`)

Tests:
- `file`: PostgreSQL directory structure
- `template`: postgresql.conf and pg_hba.conf
- `copy`: Backup scripts and initialization SQL
- `blockinfile`: Adding configuration blocks
- `lineinfile`: Tuning individual settings
- `stat`: Configuration file verification

### 3. User Management (`03_user_management.yml`)

Tests:
- `user`: System user creation
- `file`: SSH directory structure
- `copy`: authorized_keys and profile files
- `template`: SSH client configuration
- `command`: User verification
- Loops: Creating multiple users

### 4. Application Deployment (`04_app_deployment.yml`)

Tests:
- Complex directory structures (Capistrano-style)
- Symlink management for current release
- Template-based configuration
- Environment file generation
- Systemd service file creation
- Release cleanup (keeping N releases)
- `set_fact`: Dynamic variable creation
- Handler notifications

### 5. Multi-Tier Application (`05_multi_tier_app.yml`)

Tests:
- Multiple plays targeting different host groups
- Cross-play variable sharing
- Host group variables
- Complex orchestration across tiers
- Load balancer configuration
- Application-to-database connectivity
- Deployment verification

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUSTIBLE_E2E_DOCKER_ENABLED` | `0` | Set to `1` to enable E2E tests |
| `RUSTIBLE_E2E_VERBOSE` | `0` | Set to `1` for verbose output |
| `RUSTIBLE_E2E_SSH_USER` | `testuser` | SSH username for containers |
| `RUSTIBLE_E2E_SSH_PASS` | `testpassword` | SSH password for containers |
| `RUSTIBLE_E2E_DOCKER_COMPOSE_FILE` | auto-detected | Path to docker-compose.yml |

## Debugging

### Check Container Status

```bash
cd tests/e2e/docker
docker compose ps
docker compose logs -f
```

### SSH into a Container

```bash
ssh -p 2221 testuser@localhost  # web1
ssh -p 2223 testuser@localhost  # db1
```

### View Container Logs

```bash
docker logs rustible-e2e-web1
docker logs rustible-e2e-db1
```

### Run Tests with Maximum Verbosity

```bash
RUSTIBLE_E2E_DOCKER_ENABLED=1 \
RUSTIBLE_E2E_VERBOSE=1 \
RUST_LOG=debug \
cargo test --test e2e_docker_tests -- --nocapture 2>&1 | tee test.log
```

## CI/CD Integration

For CI environments, use the following pattern:

```yaml
# GitHub Actions example
e2e-tests:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4

    - name: Set up Docker Buildx
      uses: docker/setup-buildx-action@v3

    - name: Start E2E containers
      run: |
        cd tests/e2e/docker
        docker compose up -d --build
        sleep 15  # Wait for containers

    - name: Run E2E tests
      run: |
        export RUSTIBLE_E2E_DOCKER_ENABLED=1
        cargo test --test e2e_docker_tests -- --nocapture

    - name: Cleanup
      if: always()
      run: |
        cd tests/e2e/docker
        docker compose down -v
```

## Extending the Tests

### Adding a New Scenario

1. Create a new playbook in `playbooks/`:
   ```yaml
   # playbooks/06_new_scenario.yml
   ---
   - name: New Scenario
     hosts: all
     tasks:
       - name: Your test task
         debug:
           msg: "Testing new scenario"
   ```

2. Add a test function in `tests/e2e_docker_tests.rs`:
   ```rust
   #[tokio::test]
   async fn test_e2e_docker_new_scenario() {
       // ... test implementation
   }
   ```

### Adding a New Container Type

1. Create a Dockerfile in `docker/`:
   ```dockerfile
   FROM ubuntu:24.04
   # ... container setup
   ```

2. Add the service to `docker-compose.yml`

3. Update the test configuration in the Rust test file

## Troubleshooting

### "Connection refused" errors

Ensure containers are running and SSH is available:
```bash
docker compose ps
nc -zv localhost 2221
```

### "Permission denied" errors

Check that the testuser has proper sudo configuration:
```bash
docker exec rustible-e2e-web1 sudo -l -U testuser
```

### Slow test execution

Consider reducing the number of hosts or tasks for faster iteration:
```bash
./scripts/run-e2e-tests.sh test webserver  # Test one scenario at a time
```

### Template rendering issues

Check that templates exist in `tests/e2e/templates/` and are valid Jinja2:
```bash
ls -la tests/e2e/templates/
```

## Contributing

When adding new E2E tests:

1. Follow the existing naming convention (`NN_description.yml`)
2. Add comments explaining what modules/features are being tested
3. Include handlers for any configuration changes
4. Add verification tasks to confirm expected state
5. Update this README with the new scenario
