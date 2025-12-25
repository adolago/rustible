#!/bin/bash
# E2E Test Runner Script for Rustible
# This script manages Docker containers and runs E2E tests

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
E2E_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$(dirname "$E2E_DIR")")"
DOCKER_DIR="$E2E_DIR/docker"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed"
        exit 1
    fi

    if ! docker compose version &> /dev/null; then
        log_error "Docker Compose is not available"
        exit 1
    fi

    if ! command -v cargo &> /dev/null; then
        log_error "Cargo (Rust) is not installed"
        exit 1
    fi

    log_success "All prerequisites met"
}

# Build Docker images
build_images() {
    log_info "Building Docker images..."
    cd "$DOCKER_DIR"
    docker compose build
    log_success "Images built successfully"
}

# Start containers
start_containers() {
    log_info "Starting Docker containers..."
    cd "$DOCKER_DIR"
    docker compose up -d

    log_info "Waiting for containers to be ready..."
    sleep 5

    # Wait for SSH to be available on all containers
    local max_attempts=30
    local attempt=0

    while [ $attempt -lt $max_attempts ]; do
        local all_ready=true

        for port in 2221 2222 2223 2224 2225; do
            if ! nc -z localhost $port 2>/dev/null; then
                all_ready=false
                break
            fi
        done

        if [ "$all_ready" = true ]; then
            log_success "All containers are ready"
            return 0
        fi

        attempt=$((attempt + 1))
        sleep 1
    done

    log_warning "Some containers may not be fully ready"
}

# Stop containers
stop_containers() {
    log_info "Stopping Docker containers..."
    cd "$DOCKER_DIR"
    docker compose down -v
    log_success "Containers stopped"
}

# Show container status
show_status() {
    log_info "Container status:"
    cd "$DOCKER_DIR"
    docker compose ps
}

# Run E2E tests
run_tests() {
    local test_filter="${1:-}"

    log_info "Running E2E tests..."
    cd "$PROJECT_ROOT"

    export RUSTIBLE_E2E_DOCKER_ENABLED=1
    export RUSTIBLE_E2E_VERBOSE="${RUSTIBLE_E2E_VERBOSE:-0}"

    if [ -n "$test_filter" ]; then
        log_info "Running tests matching: $test_filter"
        cargo test --test e2e_docker_tests "$test_filter" -- --nocapture
    else
        cargo test --test e2e_docker_tests -- --nocapture
    fi

    log_success "E2E tests completed"
}

# Clean up everything
cleanup() {
    log_info "Cleaning up..."
    cd "$DOCKER_DIR"
    docker compose down -v --rmi local 2>/dev/null || true
    log_success "Cleanup complete"
}

# Show usage
usage() {
    cat << EOF
Rustible E2E Test Runner

Usage: $(basename "$0") <command> [options]

Commands:
    setup       Build images and start containers
    start       Start containers (assumes images are built)
    stop        Stop containers
    status      Show container status
    test        Run all E2E tests
    test <name> Run specific test (e.g., 'webserver')
    cleanup     Stop containers and remove images
    help        Show this help message

Examples:
    $(basename "$0") setup      # First-time setup
    $(basename "$0") test       # Run all tests
    $(basename "$0") test webserver  # Run webserver tests only
    $(basename "$0") cleanup    # Clean up everything

Environment Variables:
    RUSTIBLE_E2E_VERBOSE=1    Enable verbose output

EOF
}

# Main
case "${1:-help}" in
    setup)
        check_prerequisites
        build_images
        start_containers
        show_status
        ;;
    start)
        check_prerequisites
        start_containers
        show_status
        ;;
    stop)
        stop_containers
        ;;
    status)
        show_status
        ;;
    test)
        run_tests "${2:-}"
        ;;
    cleanup)
        cleanup
        ;;
    help|--help|-h)
        usage
        ;;
    *)
        log_error "Unknown command: $1"
        usage
        exit 1
        ;;
esac
