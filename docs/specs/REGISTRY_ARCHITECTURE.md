# Rustible Registry System Architecture

## Executive Summary

This document specifies the architecture for Rustible's package registry system, designed as a modern replacement for Ansible Galaxy. The system addresses known pain points with Galaxy (403 errors, 504 timeouts, version resolution failures) while providing a Cargo-like experience with superior dependency resolution, local caching, and mirror support.

---

## 1. Ansible Galaxy Pain Points Analysis

### 1.1 Current Issues with Ansible Galaxy

| Issue | Root Cause | Impact | Frequency |
|-------|-----------|--------|-----------|
| **403 Forbidden Errors** | Rate limiting, authentication expiry, CDN issues | Blocks CI/CD pipelines | High |
| **504 Gateway Timeouts** | Server overload, slow database queries for collections with many versions | Deployment failures | High |
| **Version Resolution Failures** | Collections with 50+ versions cause timeouts | Cannot install specific versions | Medium |
| **Incomplete Downloads** | Network interruptions, no resume support | Corrupted installations | Medium |
| **Slow Dependency Resolution** | Server-side resolution, no parallel fetching | Long install times | High |
| **Single Point of Failure** | No built-in mirror support | Complete service outages | Low |
| **Inconsistent Metadata** | Version format inconsistencies, missing checksums | Installation verification failures | Medium |

### 1.2 Technical Analysis

```
Galaxy API Flow (Current):
1. Client requests collection metadata -> Single HTTP call
2. Server resolves ALL dependencies -> Potential timeout for large dependency trees
3. Server returns download URLs -> No parallelization
4. Client downloads artifacts serially -> Slow for multiple dependencies
5. No resume capability -> Full restart on failure
```

**Key Problems:**
- Server-side dependency resolution is a bottleneck
- No client-side caching of metadata
- No incremental/delta updates
- Authentication tokens expire during long operations
- No checksum verification before download completion

---

## 2. Rustible Registry Design Philosophy

### 2.1 Core Principles

1. **Client-Side Intelligence**: Dependency resolution happens locally using a SAT solver
2. **Content-Addressable Storage**: All artifacts are identified by cryptographic hash
3. **Incremental Updates**: Only download changed metadata, not entire indices
4. **Parallel Operations**: Fetch metadata and artifacts concurrently
5. **Offline-First**: Full functionality with local cache
6. **Resilient**: Automatic retries, resume, mirror failover

### 2.2 Comparison with Cargo

| Feature | Cargo | Rustible Registry | Galaxy |
|---------|-------|-------------------|--------|
| Dependency Resolution | Client-side (SAT) | Client-side (SAT) | Server-side |
| Caching | Local registry index | Local index + artifacts | None |
| Checksums | SHA256 | BLAKE3 | MD5 (if any) |
| Parallel Downloads | Yes | Yes | No |
| Mirror Support | Yes (sparse registry) | Yes (multi-registry) | No |
| Offline Mode | Yes | Yes | No |
| Resume Downloads | Yes | Yes | No |

---

## 3. System Architecture

### 3.1 High-Level Architecture

```
                                  +------------------+
                                  |  Rustible CLI    |
                                  +--------+---------+
                                           |
                    +----------------------+----------------------+
                    |                      |                      |
           +--------v--------+    +--------v--------+    +--------v--------+
           |  Registry       |    |  Resolver       |    |  Cache          |
           |  Client         |    |  Engine         |    |  Manager        |
           +--------+--------+    +--------+--------+    +--------+--------+
                    |                      |                      |
                    |              +-------v-------+              |
                    |              |  Constraint   |              |
                    |              |  SAT Solver   |              |
                    |              +---------------+              |
                    |                                             |
           +--------v-----------------------------------------+---+
           |                    Index Store                       |
           |  (Sparse Registry Index with Incremental Updates)    |
           +------------------------------------------------------+
                                           |
                    +----------------------+----------------------+
                    |                      |                      |
           +--------v--------+    +--------v--------+    +--------v--------+
           |  Primary        |    |  Mirror 1       |    |  Mirror N       |
           |  Registry       |    |  Registry       |    |  Registry       |
           +--------+--------+    +-----------------+    +-----------------+
                    |
           +--------v--------+
           |  Content        |
           |  Delivery       |
           |  (CDN/S3)       |
           +-----------------+
```

### 3.2 Component Details

#### 3.2.1 Registry Client (`src/registry/client.rs`)

```rust
/// Registry client for fetching packages and metadata
pub struct RegistryClient {
    /// HTTP client with retry and timeout configuration
    http: reqwest::Client,
    /// Registry configuration (URLs, auth, mirrors)
    config: RegistryConfig,
    /// Local cache for metadata and artifacts
    cache: Arc<CacheManager>,
    /// Rate limiter to prevent 403 errors
    rate_limiter: RateLimiter,
}

impl RegistryClient {
    /// Fetch package index with incremental updates
    pub async fn fetch_index(&self, since: Option<DateTime<Utc>>) -> Result<IndexUpdate>;

    /// Download package artifact with resume support
    pub async fn download_package(&self, pkg: &PackageId) -> Result<PathBuf>;

    /// Verify package checksum
    pub async fn verify(&self, pkg: &PackageId, path: &Path) -> Result<bool>;
}
```

#### 3.2.2 Resolver Engine (`src/registry/resolver.rs`)

```rust
/// SAT-based dependency resolver (inspired by Cargo's resolver)
pub struct Resolver {
    /// Package index containing all available versions
    index: PackageIndex,
    /// Constraint solver
    solver: SatSolver,
    /// Resolution preferences (newest, minimal, locked)
    preferences: ResolverPreferences,
}

impl Resolver {
    /// Resolve dependencies to concrete versions
    pub fn resolve(&self, requirements: &[Requirement]) -> Result<Resolution>;

    /// Check if a resolution is possible without downloading
    pub fn can_resolve(&self, requirements: &[Requirement]) -> bool;

    /// Generate a lockfile from resolution
    pub fn to_lockfile(&self, resolution: &Resolution) -> Lockfile;
}

/// Resolved dependency graph
pub struct Resolution {
    /// Packages to install in topological order
    pub packages: Vec<ResolvedPackage>,
    /// Resolution metadata
    pub metadata: ResolutionMetadata,
}
```

#### 3.2.3 Cache Manager (`src/registry/cache.rs`)

```rust
/// Content-addressable cache for packages and metadata
pub struct CacheManager {
    /// Root cache directory (~/.rustible/cache)
    root: PathBuf,
    /// Index cache (compressed, incremental)
    index_cache: IndexCache,
    /// Artifact cache (content-addressable)
    artifact_cache: ArtifactCache,
    /// Cache configuration
    config: CacheConfig,
}

impl CacheManager {
    /// Get cached artifact by content hash
    pub fn get_artifact(&self, hash: &Blake3Hash) -> Option<PathBuf>;

    /// Store artifact with verification
    pub fn store_artifact(&self, data: &[u8]) -> Result<Blake3Hash>;

    /// Prune cache to stay within size limits
    pub fn prune(&self, max_size: u64) -> Result<PruneStats>;

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats;
}
```

### 3.3 Data Flow

```
Install Request Flow:
=====================

1. Parse .rustible.toml manifest
   |
   v
2. Load local index cache (or fetch if stale)
   |
   +---> If cache miss: Fetch sparse index from registry
   |                    (Only packages mentioned in requirements)
   v
3. Run SAT solver for dependency resolution
   |
   +---> If conflict: Report detailed conflict information
   |
   v
4. Generate download plan
   |
   +---> Check local cache for already-downloaded artifacts
   |
   v
5. Parallel download of missing artifacts
   |
   +---> Retry with exponential backoff on failure
   +---> Mirror failover on persistent failure
   |
   v
6. Verify checksums (BLAKE3)
   |
   v
7. Extract and install packages
   |
   v
8. Update lockfile
```

---

## 4. .rustible.toml Manifest Specification

### 4.1 Full Schema

```toml
# .rustible.toml - Rustible Package Manifest

# ============================================================
# PROJECT METADATA
# ============================================================

[project]
name = "my-infrastructure"
version = "1.0.0"
description = "Infrastructure automation for production environment"
authors = ["DevOps Team <devops@example.com>"]
license = "MIT"
readme = "README.md"
repository = "https://github.com/example/my-infrastructure"

# Minimum Rustible version required
rustible-version = ">=0.2.0"

# ============================================================
# DEPENDENCIES
# ============================================================

[dependencies]
# Simple version requirement
nginx = "2.1"

# Version with constraints
postgresql = ">=14.0, <16.0"

# From specific registry
redis = { version = "7.0", registry = "internal" }

# From Git repository
custom-role = { git = "https://github.com/org/custom-role", tag = "v1.2.3" }

# From local path (for development)
local-role = { path = "../my-local-role" }

# With optional features
kubernetes = { version = "1.28", features = ["helm", "operators"] }

# Platform-specific dependency
[dependencies.windows-base]
version = "1.0"
platforms = ["windows"]

# ============================================================
# DEV DEPENDENCIES (for testing/development only)
# ============================================================

[dev-dependencies]
molecule = "6.0"
testinfra = "8.0"

# ============================================================
# COLLECTIONS (Ansible Galaxy compatibility)
# ============================================================

[collections]
# Import existing Ansible collections with namespace
community-general = { version = "8.0", namespace = "community.general" }
amazon-aws = { version = "7.0", namespace = "amazon.aws" }

# ============================================================
# REGISTRIES
# ============================================================

[registries]
# Primary registry (default)
rustible = { url = "https://registry.rustible.io", default = true }

# Internal/private registry
internal = { url = "https://registry.internal.example.com", token-env = "INTERNAL_REGISTRY_TOKEN" }

# Mirror for fallback
mirror = { url = "https://mirror.rustible.io", priority = 10 }

# Ansible Galaxy bridge (for legacy compatibility)
galaxy = { url = "https://galaxy.ansible.com", type = "galaxy", priority = 100 }

# ============================================================
# FEATURES (optional functionality)
# ============================================================

[features]
default = ["core"]
core = []
monitoring = ["prometheus", "grafana"]
security = ["vault", "ssl-certs"]
full = ["core", "monitoring", "security"]

# Feature-gated dependencies
[features.monitoring.dependencies]
prometheus = "2.0"
grafana = "10.0"

# ============================================================
# PROFILES (environment-specific configuration)
# ============================================================

[profile.production]
# Override dependency versions for production
[profile.production.dependencies]
nginx = "=2.1.3"  # Pin exact version

[profile.development]
# Use latest for development
[profile.development.dependencies]
nginx = "*"

# ============================================================
# OVERRIDES (force specific versions)
# ============================================================

[overrides]
# Force specific version of transitive dependency
openssl = "=3.1.4"

# Replace a dependency entirely
legacy-module = { replace-with = "modern-module", version = "2.0" }

# ============================================================
# BUILD/INSTALL SETTINGS
# ============================================================

[install]
# Default install location
path = "./roles"

# Parallel downloads
parallel = 8

# Retry configuration
retry-count = 3
retry-delay = "1s"

# Timeout settings
connect-timeout = "10s"
read-timeout = "60s"

# Verification
verify-checksums = true
verify-signatures = false  # Requires GPG setup

# ============================================================
# PATCH SECTION (apply patches to dependencies)
# ============================================================

[patch.nginx]
# Apply local patch file
patches = ["patches/nginx-custom-config.patch"]

# ============================================================
# WORKSPACE (for multi-project setups)
# ============================================================

[workspace]
members = [
    "projects/web-tier",
    "projects/database-tier",
    "projects/cache-tier",
]

# Shared dependencies across workspace
[workspace.dependencies]
common-handlers = "1.0"
```

### 4.2 Lockfile Specification (`.rustible.lock`)

```toml
# .rustible.lock - Auto-generated, do not edit manually
# This file ensures reproducible installations

[[package]]
name = "nginx"
version = "2.1.3"
source = "registry+https://registry.rustible.io"
checksum = "blake3:a1b2c3d4e5f6789..."
dependencies = ["common-handlers 1.0.0"]

[[package]]
name = "postgresql"
version = "15.2.0"
source = "registry+https://registry.rustible.io"
checksum = "blake3:f6e5d4c3b2a1..."
dependencies = []

[[package]]
name = "custom-role"
version = "1.2.3"
source = "git+https://github.com/org/custom-role?tag=v1.2.3"
checksum = "blake3:1234567890ab..."

[metadata]
rustible-version = "0.2.0"
resolved-at = "2024-01-15T10:30:00Z"
```

---

## 5. Registry Server Architecture

### 5.1 API Specification

```yaml
openapi: 3.0.0
info:
  title: Rustible Registry API
  version: 1.0.0

paths:
  /api/v1/index/{package}:
    get:
      summary: Get package index (sparse registry)
      parameters:
        - name: package
          in: path
          required: true
          schema:
            type: string
      responses:
        200:
          description: Package index
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/PackageIndex'
        304:
          description: Not modified (ETag match)

  /api/v1/index/changes:
    get:
      summary: Get incremental index updates
      parameters:
        - name: since
          in: query
          schema:
            type: string
            format: date-time
      responses:
        200:
          description: Index changes since timestamp

  /api/v1/packages/{name}/{version}:
    get:
      summary: Get package metadata
      responses:
        200:
          description: Package metadata with download URL

  /api/v1/download/{name}/{version}:
    get:
      summary: Download package artifact
      responses:
        200:
          description: Package tarball
        302:
          description: Redirect to CDN

components:
  schemas:
    PackageIndex:
      type: object
      properties:
        name:
          type: string
        versions:
          type: array
          items:
            $ref: '#/components/schemas/VersionEntry'

    VersionEntry:
      type: object
      properties:
        version:
          type: string
        checksum:
          type: string
        dependencies:
          type: array
          items:
            $ref: '#/components/schemas/Dependency'
        yanked:
          type: boolean
```

### 5.2 Sparse Registry Index Format

Inspired by Cargo's sparse registry protocol, each package has its own index file:

```
registry-index/
  ni/
    ng/
      nginx          # Contains all nginx versions
  po/
    st/
      postgresql     # Contains all postgresql versions
  config.json        # Registry configuration
```

**Package Index File Format (nginx):**

```json
{"name":"nginx","vers":"1.0.0","deps":[],"cksum":"blake3:...","features":{}}
{"name":"nginx","vers":"1.1.0","deps":[],"cksum":"blake3:...","features":{}}
{"name":"nginx","vers":"2.0.0","deps":[{"name":"common","req":">=1.0"}],"cksum":"blake3:...","features":{"ssl":["openssl"]}}
{"name":"nginx","vers":"2.1.0","deps":[{"name":"common","req":">=1.0"}],"cksum":"blake3:...","features":{"ssl":["openssl"]}}
{"name":"nginx","vers":"2.1.3","deps":[{"name":"common","req":">=1.0"}],"cksum":"blake3:...","features":{"ssl":["openssl"]}}
```

Benefits:
- Only fetch index files for packages actually needed
- Cacheable with HTTP ETags
- Efficient for registries with thousands of packages
- Append-only format (new versions add lines)

---

## 6. Mirror and Fallback Strategy

### 6.1 Multi-Registry Resolution

```rust
/// Registry selection strategy
pub enum RegistryStrategy {
    /// Try registries in priority order
    Priority,
    /// Use fastest responding registry
    Fastest,
    /// Load balance across registries
    RoundRobin,
    /// Primary with automatic failover
    Failover { max_retries: u32 },
}

/// Mirror configuration
pub struct MirrorConfig {
    /// Mirror URL
    pub url: String,
    /// Priority (lower = higher priority)
    pub priority: u32,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Maximum concurrent requests
    pub max_concurrent: u32,
}
```

### 6.2 Failover Algorithm

```
Failover Process:
=================

1. Attempt request to primary registry
   |
   +---> Success: Return result
   |
   +---> Failure (timeout/5xx):
         |
         v
2. Mark primary as unhealthy (circuit breaker)
   |
   v
3. Try next mirror in priority order
   |
   +---> Success: Return result, keep primary marked unhealthy
   |
   +---> All mirrors fail: Return error with details

4. Background health check on primary
   |
   +---> Healthy: Reset circuit breaker
   |
   +---> Still unhealthy: Extend circuit breaker timeout
```

---

## 7. Caching Architecture

### 7.1 Cache Layout

```
~/.rustible/
  cache/
    registry/
      index/
        rustible.io/
          ni/ng/nginx.idx      # Sparse index files
          po/st/postgresql.idx
          last-update.json     # Timestamp for incremental updates
      artifacts/
        blake3/
          a1/b2/a1b2c3d4...    # Content-addressable storage
      downloads/
        nginx-2.1.3.tar.gz.partial  # Incomplete downloads (for resume)
    git/
      github.com/
        org/
          custom-role/
            1.2.3/             # Git checkouts by version
```

### 7.2 Cache Policies

```rust
/// Cache configuration
pub struct CacheConfig {
    /// Maximum cache size (0 = unlimited)
    pub max_size: u64,

    /// Maximum age for index cache
    pub index_max_age: Duration,

    /// Maximum age for artifacts (never expire by default)
    pub artifact_max_age: Option<Duration>,

    /// Eviction policy when cache is full
    pub eviction_policy: EvictionPolicy,

    /// Verify checksums on cache hit
    pub verify_on_hit: bool,
}

pub enum EvictionPolicy {
    /// Least Recently Used
    Lru,
    /// Least Frequently Used
    Lfu,
    /// First In First Out
    Fifo,
    /// Remove oldest versions first
    OldestVersion,
}
```

---

## 8. Security Considerations

### 8.1 Package Verification

```rust
/// Package verification options
pub struct VerificationConfig {
    /// Require checksum verification (default: true)
    pub require_checksum: bool,

    /// Checksum algorithm
    pub checksum_algorithm: ChecksumAlgorithm,

    /// Require GPG signature (default: false)
    pub require_signature: bool,

    /// Trusted GPG keys
    pub trusted_keys: Vec<GpgKeyId>,

    /// Reject yanked packages (default: true)
    pub reject_yanked: bool,
}

pub enum ChecksumAlgorithm {
    Blake3,      // Default, fastest
    Sha256,      // Compatibility
    Sha512,      // Maximum security
}
```

### 8.2 Authentication

```rust
/// Registry authentication
pub enum AuthMethod {
    /// No authentication (public registries)
    None,

    /// Bearer token (environment variable)
    Token { env_var: String },

    /// HTTP Basic Auth
    Basic { username: String, password_env: String },

    /// OAuth2 client credentials
    OAuth2 {
        token_url: String,
        client_id: String,
        client_secret_env: String,
    },
}
```

---

## 9. CLI Commands

### 9.1 Registry Commands

```bash
# Initialize a new manifest
rustible init

# Add a dependency
rustible add nginx
rustible add nginx@2.1
rustible add nginx --git https://github.com/org/nginx-role

# Remove a dependency
rustible remove nginx

# Install dependencies
rustible install
rustible install --locked  # Use lockfile exactly
rustible install --offline # Use only cache

# Update dependencies
rustible update          # Update all to latest compatible
rustible update nginx    # Update specific package
rustible update --breaking # Allow breaking updates

# Show dependency tree
rustible tree
rustible tree --duplicates

# Search packages
rustible search nginx
rustible search --category database

# Publish a package
rustible publish
rustible publish --dry-run

# Cache management
rustible cache clean
rustible cache clean --expired
rustible cache stats

# Registry management
rustible registry add internal https://registry.internal.com
rustible registry remove internal
rustible registry list
```

### 9.2 Command Examples

```bash
$ rustible install
Updating registry index...
Resolving dependencies... done (0.3s)
Downloading 12 packages...
  [1/12] nginx 2.1.3 ................. done
  [2/12] postgresql 15.2.0 ........... done
  [3/12] redis 7.0.0 ................. done
  ...
Installed 12 packages in 4.2s

$ rustible tree
my-infrastructure v1.0.0
+-- nginx v2.1.3
|   +-- common-handlers v1.0.0
+-- postgresql v15.2.0
+-- redis v7.0.0
    +-- common-handlers v1.0.0

$ rustible cache stats
Cache Statistics:
  Index cache: 2.3 MB (15 packages)
  Artifact cache: 156 MB (45 versions)
  Last updated: 2 hours ago
  Hit rate: 89%
```

---

## 10. Ansible Galaxy Compatibility Bridge

### 10.1 Galaxy Import

```rust
/// Import Galaxy collection into Rustible format
pub struct GalaxyBridge {
    /// Galaxy API client
    galaxy_client: GalaxyApiClient,
    /// Target registry for publishing
    target_registry: RegistryClient,
}

impl GalaxyBridge {
    /// Import a Galaxy collection
    pub async fn import(&self, namespace: &str, name: &str) -> Result<Package>;

    /// Convert Galaxy metadata to Rustible format
    fn convert_metadata(&self, galaxy_meta: GalaxyMeta) -> PackageMeta;

    /// Map Galaxy dependencies to Rustible requirements
    fn map_dependencies(&self, deps: Vec<GalaxyDep>) -> Vec<Requirement>;
}
```

### 10.2 requirements.yml Compatibility

```yaml
# Legacy requirements.yml (still supported)
collections:
  - name: community.general
    version: ">=8.0.0"
  - name: amazon.aws
```

```bash
# Convert to .rustible.toml
rustible import requirements.yml
```

---

## 11. Implementation Roadmap

### Phase 1: Core Infrastructure (Weeks 1-4)
- [ ] Manifest parser (.rustible.toml)
- [ ] Local cache manager (content-addressable)
- [ ] Basic HTTP client with retry logic
- [ ] Package extraction and installation

### Phase 2: Dependency Resolution (Weeks 5-8)
- [ ] SAT solver integration (using resolvo or pubgrub)
- [ ] Version constraint parsing
- [ ] Lockfile generation and parsing
- [ ] Dependency tree visualization

### Phase 3: Registry Client (Weeks 9-12)
- [ ] Sparse registry protocol implementation
- [ ] Incremental index updates
- [ ] Multi-registry support
- [ ] Mirror failover

### Phase 4: Polish and Compatibility (Weeks 13-16)
- [ ] Galaxy bridge for backwards compatibility
- [ ] CLI improvements
- [ ] Documentation
- [ ] Performance optimization

---

## 12. References

1. Cargo Registry Protocol: https://doc.rust-lang.org/cargo/reference/registry-index.html
2. Cargo Sparse Registry RFC: https://rust-lang.github.io/rfcs/2789-sparse-index.html
3. PubGrub version solving: https://github.com/pubgrub-rs/pubgrub
4. BLAKE3 hash function: https://github.com/BLAKE3-team/BLAKE3
5. Ansible Galaxy API: https://galaxy.ansible.com/api/docs/

---

## Appendix A: Error Codes

| Code | Description | Recovery Action |
|------|-------------|-----------------|
| E001 | Registry unreachable | Retry with mirror |
| E002 | Package not found | Check spelling, search |
| E003 | Version not found | Use `rustible search` |
| E004 | Dependency conflict | Review constraints |
| E005 | Checksum mismatch | Re-download, report |
| E006 | Authentication failed | Check token/credentials |
| E007 | Rate limited | Wait and retry |
| E008 | Disk space exhausted | Prune cache |

---

## Appendix B: Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUSTIBLE_REGISTRY` | Default registry URL | `https://registry.rustible.io` |
| `RUSTIBLE_CACHE_DIR` | Cache directory | `~/.rustible/cache` |
| `RUSTIBLE_OFFLINE` | Offline mode | `false` |
| `RUSTIBLE_NO_VERIFY` | Skip checksum verification | `false` |
| `RUSTIBLE_PARALLEL` | Parallel downloads | `8` |
| `RUSTIBLE_TOKEN` | Authentication token | - |
| `RUSTIBLE_LOG_LEVEL` | Logging level | `info` |
