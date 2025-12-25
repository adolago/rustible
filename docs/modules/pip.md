# pip - Manage Python Packages

## Synopsis

The `pip` module manages Python packages using pip. It can install, remove, and upgrade Python packages from PyPI or other sources, with support for virtual environments, requirements files, and proxy configurations.

## Classification

**RemoteCommand** - This module executes pip commands on remote hosts via SSH.

## Parameters

| Parameter | Required | Default | Type | Description |
|-----------|----------|---------|------|-------------|
| `name` | yes* | - | string/list | Package name(s) to manage. Can include version specifiers (e.g., `flask>=2.0`). |
| `state` | no | present | string | Desired state: present, absent, latest, forcereinstall. |
| `version` | no | - | string | Specific version to install. Supports specifiers like `>=2.0` or `<3.0,>=2.5`. |
| `requirements` | no | - | string | Path to a requirements.txt file. |
| `virtualenv` | no | - | string | Path to a virtualenv to use. Created if it does not exist. |
| `virtualenv_command` | no | python3 -m venv | string | Command to create virtualenv. |
| `virtualenv_python` | no | - | string | Python interpreter to use for virtualenv creation. |
| `virtualenv_site_packages` | no | false | boolean | Allow virtualenv to access system site-packages. |
| `executable` | no | pip3 | string | The pip executable to use. |
| `extra_args` | no | - | string/list | Extra arguments to pass to pip (e.g., `--trusted-host`, `--no-cache-dir`). |
| `editable` | no | false | boolean | Install package in editable/development mode (-e flag). |
| `chdir` | no | - | string | Directory to change to before running pip. |
| `umask` | no | - | string/int | Umask to apply during installation (e.g., `0022` or `0o077`). |
| `proxy` | no | - | string | Proxy URL to use for pip operations. |
| `index_url` | no | - | string | Custom PyPI index URL (--index-url). |
| `extra_index_url` | no | - | string | Additional PyPI index URL (--extra-index-url). |
| `no_index` | no | false | boolean | Ignore package index (only use --find-links). |
| `find_links` | no | - | string | URL or path to look for packages (--find-links). |

*Required unless using `requirements`.

## State Values

| State | Description |
|-------|-------------|
| `present` | Ensure the package is installed |
| `absent` | Ensure the package is removed |
| `latest` | Ensure the package is at the latest version |
| `forcereinstall` | Reinstall the package even if already present |

## Return Values

| Key | Type | Description |
|-----|------|-------------|
| `name` | string/list | Package name(s) managed |
| `version` | string | Version installed (if specified) |
| `virtualenv` | string | Virtualenv path (if used) |
| `stdout` | string | Standard output from pip |
| `stderr` | string | Standard error from pip |
| `results` | object | Per-package status (installed, removed, ok) |

## Examples

### Install a package

```yaml
- name: Install Flask
  pip:
    name: flask
    state: present
```

### Install a specific version

```yaml
- name: Install Django 4.2
  pip:
    name: django
    version: "4.2"
```

### Install with version range

```yaml
- name: Install requests with version constraints
  pip:
    name: requests
    version: ">=2.25,<3.0"
```

### Install multiple packages

```yaml
- name: Install web dependencies
  pip:
    name:
      - flask
      - sqlalchemy
      - gunicorn
    state: present
```

### Install from requirements file

```yaml
- name: Install project dependencies
  pip:
    requirements: /opt/myapp/requirements.txt
```

### Install in a virtualenv

```yaml
- name: Install in virtualenv
  pip:
    name: flask
    virtualenv: /opt/myapp/venv
    virtualenv_python: python3.11
```

### Create virtualenv with system packages

```yaml
- name: Install with access to system packages
  pip:
    name: numpy
    virtualenv: /opt/scientific/venv
    virtualenv_site_packages: yes
```

### Install in editable mode

```yaml
- name: Install local package in editable mode
  pip:
    name: /path/to/mypackage
    editable: yes
```

### Upgrade a package

```yaml
- name: Upgrade pip itself
  pip:
    name: pip
    state: latest
```

### Force reinstall a package

```yaml
- name: Force reinstall package
  pip:
    name: mypackage
    state: forcereinstall
```

### Remove a package

```yaml
- name: Uninstall package
  pip:
    name: deprecated-package
    state: absent
```

### Use pip3 explicitly

```yaml
- name: Use pip3
  pip:
    name: flask
    executable: pip3
```

### Install with extra arguments

```yaml
- name: Install with trusted host
  pip:
    name: private-package
    extra_args: --trusted-host pypi.internal.example.com --no-cache-dir
```

### Install with extra arguments as list

```yaml
- name: Install with multiple extra args
  pip:
    name: private-package
    extra_args:
      - --trusted-host
      - pypi.internal.example.com
      - --no-cache-dir
```

### Install using a proxy

```yaml
- name: Install via corporate proxy
  pip:
    name: requests
    proxy: http://proxy.example.com:8080
```

### Install from private PyPI

```yaml
- name: Install from private index
  pip:
    name: internal-package
    index_url: https://pypi.internal.example.com/simple
```

### Install with custom index and fallback

```yaml
- name: Install with primary and fallback index
  pip:
    name: mypackage
    index_url: https://pypi.internal.example.com/simple
    extra_index_url: https://pypi.org/simple
```

### Install from local packages only

```yaml
- name: Install from local wheel files
  pip:
    name: mypackage
    no_index: yes
    find_links: /opt/packages/wheels
```

### Install with specific umask

```yaml
- name: Install with restricted permissions
  pip:
    name: private-app
    umask: "0077"
```

### Change directory before install

```yaml
- name: Install from specific directory
  pip:
    name: .
    chdir: /opt/myapp
    editable: yes
```

## Notes

- Package names are validated to prevent command injection
- When using `virtualenv`, the virtualenv is created if it does not exist
- The `requirements` parameter reads packages from a pip requirements file
- Version specifications like `>=1.0,<2.0` are supported in both `name` and `version` parameters
- Consider using `virtualenv` for isolation in production environments
- The `proxy` parameter sets the proxy for pip operations; alternatively use environment variables
- The `extra_args` parameter can be a space-separated string or a list of arguments
- The `umask` parameter can be specified as an octal string (e.g., "0022") or with "0o" prefix (e.g., "0o077")

## Virtual Environment Options

The module supports three ways to create virtual environments:

1. **python3 -m venv** (default): Uses Python's built-in venv module
2. **virtualenv**: Uses the virtualenv package with more options
3. **Custom command**: Specify via `virtualenv_command`

When using `virtualenv_python`, the specified Python interpreter is used:
- With `python3 -m venv`: The interpreter runs the venv module
- With `virtualenv`: Uses `--python` flag to select interpreter

## Proxy Configuration

Proxy can be configured in several ways:

1. **Module parameter**: `proxy: http://proxy.example.com:8080`
2. **Environment variables**: `HTTP_PROXY`, `HTTPS_PROXY`
3. **pip configuration file**: `~/.config/pip/pip.conf`

The module parameter takes precedence over environment variables.

## See Also

- [package](package.md) - Generic package manager
- [apt](apt.md) - System package management (for python3-pip)
