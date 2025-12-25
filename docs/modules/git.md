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

## See Also

- [command](command.md) - For running custom git commands
- [file](file.md) - For managing repository directory permissions
