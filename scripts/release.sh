#!/usr/bin/env bash
#
# Release script for Rustible
# Usage: ./scripts/release.sh <version> [--dry-run]
#
# Examples:
#   ./scripts/release.sh 1.0.0
#   ./scripts/release.sh 1.0.0-beta.1 --dry-run
#   ./scripts/release.sh patch
#   ./scripts/release.sh minor
#   ./scripts/release.sh major

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

usage() {
    cat << EOF
Usage: $(basename "$0") <version> [options]

Arguments:
  version       Version to release. Can be:
                - Explicit version: 1.0.0, 1.0.0-beta.1
                - Bump type: major, minor, patch

Options:
  --dry-run     Show what would be done without making changes
  --no-push     Create tag but don't push to remote
  --help        Show this help message

Examples:
  $(basename "$0") 1.0.0
  $(basename "$0") 1.0.0-rc.1
  $(basename "$0") patch
  $(basename "$0") minor --dry-run

EOF
    exit 1
}

# Parse arguments
DRY_RUN=false
NO_PUSH=false
VERSION=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --no-push)
            NO_PUSH=true
            shift
            ;;
        --help|-h)
            usage
            ;;
        -*)
            log_error "Unknown option: $1"
            usage
            ;;
        *)
            if [[ -z "$VERSION" ]]; then
                VERSION="$1"
            else
                log_error "Too many arguments"
                usage
            fi
            shift
            ;;
    esac
done

if [[ -z "$VERSION" ]]; then
    log_error "Version is required"
    usage
fi

cd "$PROJECT_ROOT"

# Get current version from Cargo.toml
get_current_version() {
    grep -m1 '^version' Cargo.toml | sed 's/.*"\([^"]*\)".*/\1/'
}

# Bump version based on type
bump_version() {
    local current="$1"
    local type="$2"

    local major minor patch
    major=$(echo "$current" | cut -d. -f1)
    minor=$(echo "$current" | cut -d. -f2)
    patch=$(echo "$current" | cut -d. -f3 | cut -d- -f1)

    case "$type" in
        major)
            echo "$((major + 1)).0.0"
            ;;
        minor)
            echo "${major}.$((minor + 1)).0"
            ;;
        patch)
            echo "${major}.${minor}.$((patch + 1))"
            ;;
        *)
            echo "$type"  # Assume explicit version
            ;;
    esac
}

# Validate semver format
validate_semver() {
    local version="$1"
    if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9]+(\.[a-zA-Z0-9]+)*)?$ ]]; then
        log_error "Invalid semantic version format: $version"
        exit 1
    fi
}

# Update version in Cargo.toml
update_cargo_toml() {
    local new_version="$1"
    local base_version
    base_version=$(echo "$new_version" | cut -d- -f1)

    if $DRY_RUN; then
        log_info "[DRY-RUN] Would update Cargo.toml version to: $base_version"
    else
        sed -i "s/^version = \".*\"/version = \"$base_version\"/" Cargo.toml
        log_success "Updated Cargo.toml version to: $base_version"
    fi
}

# Update Cargo.lock
update_cargo_lock() {
    if $DRY_RUN; then
        log_info "[DRY-RUN] Would update Cargo.lock"
    else
        cargo update --package rustible
        log_success "Updated Cargo.lock"
    fi
}

# Generate changelog
generate_changelog() {
    local version="$1"

    if ! command -v git-cliff &> /dev/null; then
        log_warn "git-cliff not installed. Installing..."
        if $DRY_RUN; then
            log_info "[DRY-RUN] Would install git-cliff"
        else
            cargo install git-cliff
        fi
    fi

    if $DRY_RUN; then
        log_info "[DRY-RUN] Would generate changelog for v$version"
    else
        git-cliff --config cliff.toml --tag "v$version" -o CHANGELOG.md
        log_success "Generated CHANGELOG.md"
    fi
}

# Create git commit and tag
create_release_commit() {
    local version="$1"

    if $DRY_RUN; then
        log_info "[DRY-RUN] Would commit release changes"
        log_info "[DRY-RUN] Would create tag: v$version"
    else
        git add Cargo.toml Cargo.lock CHANGELOG.md
        git commit -m "chore(release): prepare v$version

- Update version in Cargo.toml
- Generate changelog
- Prepare release artifacts"

        git tag -a "v$version" -m "Release v$version

See CHANGELOG.md for release notes."

        log_success "Created commit and tag for v$version"
    fi
}

# Push to remote
push_release() {
    local version="$1"

    if $NO_PUSH; then
        log_warn "Skipping push (--no-push specified)"
        return
    fi

    if $DRY_RUN; then
        log_info "[DRY-RUN] Would push main branch and tag v$version"
    else
        git push origin main
        git push origin "v$version"
        log_success "Pushed release to remote"
    fi
}

# Main release flow
main() {
    log_info "Starting release process..."

    # Check for uncommitted changes
    if ! git diff-index --quiet HEAD --; then
        log_error "There are uncommitted changes. Please commit or stash them first."
        exit 1
    fi

    # Ensure we're on main branch
    local current_branch
    current_branch=$(git branch --show-current)
    if [[ "$current_branch" != "main" ]]; then
        log_warn "Not on main branch (current: $current_branch)"
        read -p "Continue anyway? [y/N] " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi

    # Get current and new version
    local current_version new_version
    current_version=$(get_current_version)
    new_version=$(bump_version "$current_version" "$VERSION")

    # Validate version
    validate_semver "$new_version"

    log_info "Current version: $current_version"
    log_info "New version: $new_version"

    if $DRY_RUN; then
        log_warn "DRY RUN MODE - No changes will be made"
    fi

    echo
    read -p "Proceed with release v$new_version? [y/N] " -n 1 -r
    echo

    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Release cancelled"
        exit 0
    fi

    # Execute release steps
    update_cargo_toml "$new_version"
    update_cargo_lock
    generate_changelog "$new_version"
    create_release_commit "$new_version"
    push_release "$new_version"

    echo
    log_success "Release v$new_version complete!"
    echo
    log_info "GitHub Actions will now:"
    log_info "  1. Run tests"
    log_info "  2. Build release binaries for all platforms"
    log_info "  3. Create GitHub release with artifacts"
    log_info "  4. Publish to crates.io (stable releases only)"
    echo
    log_info "Monitor progress at: https://github.com/rustible/rustible/actions"
}

main
