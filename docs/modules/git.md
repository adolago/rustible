# git - Manage Git Repositories

## Synopsis

The `git` module manages git repositories. It can clone repositories, checkout specific versions, update existing clones, and manage repository state with full support for SSH key authentication, GPG verification, and advanced git options.

## Classification

**RemoteCommand** - This module executes git commands on remote hosts via SSH.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `repo` | yes | - | string | Git repository URL (HTTPS or SSH). |
| `dest` | yes | - | string | Destination directory for the clone. |
| `version` | no | HEAD | string | Branch, tag, or commit hash to checkout. |
| `depth` | no | - | integer | Create a shallow clone with limited history. |
| `force` | no | false | boolean | Discard local modifications before updating. |
| `update` | no | true | boolean | Update the repository if it already exists. |
| `clone` | no | true | boolean | Clone if the repository does not exist. |
| `bare` | no | false | boolean | Create a bare repository (no working directory). |
| `recursive` | no | true | boolean | Clone submodules recursively. |
| `single_branch` | no | false | boolean | Clone only the specified branch history. |
| `track_submodules` | no | false | boolean | Track submodule changes from remote. |
| `remote` | no | origin | string | Name of the remote. |
| `refspec` | no | - | string | Additional refspec to fetch. |
| `separate_git_dir` | no | - | string | Store .git directory in a separate location. |
| `ssh_opts` | no | - | string | Additional SSH options for git operations. |
| `key_file` | no | - | string | SSH private key file for authentication. |
| `accept_hostkey` | no | false | boolean | Accept unknown SSH host keys automatically. |
| `gpg_whitelist` | no | - | list | List of trusted GPG key fingerprints for commit verification. |
| `verify_commit` | no | false | boolean | Verify GPG signature of the checked out commit. |
| `umask` | no | - | string | Umask for file permissions (octal, e.g., "0022"). |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `after` | string | Commit hash after the operation |
| `before` | string | Commit hash before the operation |
| `commits` | list | List of new commits (when updating) |
| `remote_url_changed` | boolean | Whether the remote URL changed |

## Examples

### Clone a repository

```yaml
- name: Clone application repository
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
```

### Clone a specific branch

```yaml
- name: Clone develop branch
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
    version: develop
```

### Clone a specific tag

```yaml
- name: Clone release tag
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
    version: v2.1.0
```

### Shallow clone for faster deployment

```yaml
- name: Shallow clone with single branch
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
    depth: 1
    single_branch: yes
```

### Clone with SSH key

```yaml
- name: Clone private repository with SSH key
  git:
    repo: git@github.com:example/private-app.git
    dest: /opt/myapp
    key_file: /home/deploy/.ssh/deploy_key
    accept_hostkey: yes
```

### Clone with custom SSH options

```yaml
- name: Clone via SSH proxy
  git:
    repo: git@github.com:example/myapp.git
    dest: /opt/myapp
    key_file: /home/deploy/.ssh/id_rsa
    ssh_opts: "-o ProxyCommand='ssh -W %h:%p jumphost'"
```

### Force update discarding local changes

```yaml
- name: Force update repository
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
    force: yes
```

### Clone without submodules

```yaml
- name: Clone without submodules
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
    recursive: no
```

### Clone specific commit

```yaml
- name: Clone specific commit
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
    version: a1b2c3d4e5f6
```

### Create a bare repository

```yaml
- name: Clone bare repository for mirroring
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp.git
    bare: yes
```

### Clone with separate git directory

```yaml
- name: Clone with separate git directory
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
    separate_git_dir: /opt/git-data/myapp.git
```

### Track submodule changes

```yaml
- name: Clone with submodule tracking
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
    recursive: yes
    track_submodules: yes
```

### Fetch specific refspec

```yaml
- name: Fetch pull request refs
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
    refspec: "+refs/pull/*/head:refs/remotes/origin/pr/*"
```

### Verify GPG signed commits

```yaml
- name: Clone with GPG verification
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
    version: v2.0.0
    verify_commit: yes
    gpg_whitelist:
      - "ABCD1234EFGH5678"
      - "9876FEDC5432BA10"
```

### Clone with custom remote name

```yaml
- name: Clone with upstream remote
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
    remote: upstream
```

### Skip cloning if not present

```yaml
- name: Only update if already cloned
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
    clone: no
    update: yes
```

### Set file permissions

```yaml
- name: Clone with specific umask
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
    umask: "0027"
```

### Check out and register result

```yaml
- name: Clone and track version
  git:
    repo: https://github.com/example/myapp.git
    dest: /opt/myapp
  register: git_result

- name: Show what was deployed
  debug:
    msg: "Deployed from {{ git_result.before }} to {{ git_result.after }}"
  when: git_result.changed

- name: List new commits
  debug:
    msg: "New commits: {{ git_result.commits | join(', ') }}"
  when: git_result.commits | length > 0
```

### Complete deployment example

```yaml
- name: Deploy application from git
  git:
    repo: git@github.com:example/myapp.git
    dest: /opt/myapp
    version: "{{ app_version }}"
    key_file: /home/deploy/.ssh/deploy_key
    accept_hostkey: yes
    force: yes
    depth: 1
    single_branch: yes
  register: deploy_result

- name: Restart application if updated
  systemd:
    name: myapp
    state: restarted
  when: deploy_result.changed
```

## Notes

- Git must be installed on the target system
- SSH URLs require proper SSH key configuration
- HTTPS URLs may require credentials for private repositories
- The `force` option will discard uncommitted local changes and clean untracked files
- The module is idempotent; it will not clone if the repository is already at the desired version
- Use `depth: 1` with `single_branch: yes` for fastest clones when you do not need full history
- The `key_file` option sets `IdentitiesOnly=yes` to ensure only the specified key is used
- The `accept_hostkey` option disables strict host key checking - use with caution
- GPG verification requires gpg to be installed and the signing keys to be in the keyring
- The `umask` parameter only affects file permissions on Unix-like systems
- Bare repositories are typically used for mirroring or as remote repositories

## Diff Output

The module provides detailed diff output showing:
- Current commit hash (abbreviated)
- Current branch name
- Local changes (if any)
- New commits when updating (up to 10 shown)

Example diff output:
```
--- before
+++ after
@@ @@
-commit: a1b2c3d4
-branch: main
+commit: e5f6g7h8
+branch: main
+
+New commits:
+  e5f6g7h8 Fix security vulnerability
+  d4c3b2a1 Update dependencies
+  c3b2a1d4 Add new feature
```

## Real-World Use Cases

### Blue-Green Deployment

```yaml
- name: Clone to new release directory
  git:
    repo: git@github.com:example/myapp.git
    dest: /opt/myapp/releases/{{ release_timestamp }}
    version: "{{ deploy_version }}"
    key_file: /home/deploy/.ssh/deploy_key
    accept_hostkey: yes
    depth: 1
  register: git_result

- name: Update current symlink
  file:
    path: /opt/myapp/current
    src: /opt/myapp/releases/{{ release_timestamp }}
    state: link
  when: git_result.changed
```

### Multi-Repository Setup

```yaml
- name: Clone application components
  git:
    repo: "{{ item.repo }}"
    dest: "/opt/{{ item.name }}"
    version: "{{ item.version | default('main') }}"
    key_file: /home/deploy/.ssh/deploy_key
  loop:
    - { name: frontend, repo: "git@github.com:example/frontend.git", version: "v2.0.0" }
    - { name: backend, repo: "git@github.com:example/backend.git", version: "v1.5.0" }
    - { name: shared, repo: "git@github.com:example/shared-lib.git" }
```

### Development Environment

```yaml
- name: Clone with full history for development
  git:
    repo: https://github.com/example/myapp.git
    dest: /home/{{ user }}/projects/myapp
    version: develop
    update: yes
    force: no  # Don't overwrite local changes
```

## Troubleshooting

### Permission denied (publickey)

SSH key authentication issues:

```bash
# Test SSH connection manually
ssh -T git@github.com

# Check key permissions
ls -la ~/.ssh/
chmod 600 ~/.ssh/id_rsa
chmod 644 ~/.ssh/id_rsa.pub
```

Use `key_file` and `accept_hostkey`:

```yaml
- git:
    repo: git@github.com:example/repo.git
    dest: /opt/repo
    key_file: /home/deploy/.ssh/deploy_key
    accept_hostkey: yes
```

### Host key verification failed

First time connecting to a git host:

```yaml
# Option 1: Accept hostkey automatically (less secure)
- git:
    repo: git@github.com:example/repo.git
    accept_hostkey: yes

# Option 2: Pre-populate known_hosts (more secure)
- name: Add GitHub to known_hosts
  known_hosts:
    name: github.com
    key: "{{ lookup('pipe', 'ssh-keyscan github.com 2>/dev/null') }}"
```

### Local changes would be overwritten

The repository has uncommitted changes. Use `force: yes` to discard them:

```yaml
- git:
    repo: https://github.com/example/repo.git
    dest: /opt/repo
    force: yes  # WARNING: This discards local changes
```

Or stash changes first:

```yaml
- name: Stash local changes
  command: git stash
  args:
    chdir: /opt/repo
  ignore_errors: yes

- name: Update repository
  git:
    repo: https://github.com/example/repo.git
    dest: /opt/repo
```

### Clone is very slow

Use shallow clones for faster performance:

```yaml
- git:
    repo: https://github.com/example/large-repo.git
    dest: /opt/repo
    depth: 1
    single_branch: yes
```

### Submodule not initialized

Ensure `recursive: yes` is set (default):

```yaml
- git:
    repo: https://github.com/example/repo.git
    dest: /opt/repo
    recursive: yes
```

Or initialize manually:

```yaml
- name: Initialize submodules
  command: git submodule update --init --recursive
  args:
    chdir: /opt/repo
```

### Version/tag not found

Ensure the version exists and fetch updates:

```yaml
- name: Fetch all refs first
  git:
    repo: https://github.com/example/repo.git
    dest: /opt/repo
    update: yes

- name: Checkout specific tag
  git:
    repo: https://github.com/example/repo.git
    dest: /opt/repo
    version: v1.2.3
```

### GPG verification fails

Ensure GPG keys are in the keyring:

```bash
gpg --recv-keys KEYID
gpg --list-keys
```

```yaml
- git:
    repo: https://github.com/example/repo.git
    dest: /opt/repo
    verify_commit: yes
    gpg_whitelist:
      - "KEYFINGERPRINT"
```

## See Also

- [command](command.md) - For running custom git commands
- [file](file.md) - For managing repository directory permissions
- [copy](copy.md) - For deploying built artifacts
- [template](template.md) - For generating config files for the repo
- [service](service.md) - For restarting services after deployment
