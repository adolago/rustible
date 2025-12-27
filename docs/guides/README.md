# Rustible User Guide

Welcome to the comprehensive Rustible User Guide. This guide covers everything you need to know to effectively use Rustible for infrastructure automation.

## Table of Contents

### Getting Started
- [Quick Start](../quick-start.md) - Installation and first playbook
- [Migration from Ansible](../migration-from-ansible.md) - Transitioning from Ansible

### Core Concepts
1. [Chapter 1: Introduction](01-introduction.md)
   - What is Rustible?
   - Key features and benefits
   - Architecture overview

2. [Chapter 2: Playbooks](02-playbooks.md)
   - Playbook structure
   - Plays, tasks, and handlers
   - Using variables and templates

3. [Chapter 3: Inventory](03-inventory.md)
   - Inventory formats (YAML, INI, JSON)
   - Dynamic inventory
   - Host patterns and groups

4. [Chapter 4: Variables](04-variables.md)
   - Variable precedence
   - Special variables
   - Variable scoping

5. [Chapter 5: Modules](05-modules.md)
   - Built-in modules
   - Module parameters
   - Module classification

6. [Chapter 6: Roles](06-roles.md)
   - Role structure
   - Using roles in playbooks
   - Role dependencies

### Advanced Topics
7. [Chapter 7: Execution Strategies](07-execution-strategies.md)
   - Linear, free, and host-pinned strategies
   - Serial execution
   - Parallelization control

8. [Chapter 8: Security](08-security.md)
   - Vault encryption
   - SSH configuration
   - Privilege escalation

9. [Chapter 9: Templating](09-templating.md)
   - Jinja2 syntax
   - Filters and tests
   - Template best practices

### Practical Guides
- [Troubleshooting Guide](troubleshooting.md)
- [Best Practices Guide](best-practices.md)
- [Performance Tuning Guide](performance-tuning.md)

### Reference
- [CLI Reference](../cli-reference.md)
- [Module Reference](../modules/README.md)
- [Variable Reference](../variables.md)

## Quick Navigation

| I want to... | Go to... |
|--------------|----------|
| Install Rustible | [Quick Start](../quick-start.md) |
| Run my first playbook | [Chapter 2: Playbooks](02-playbooks.md) |
| Manage hosts | [Chapter 3: Inventory](03-inventory.md) |
| Use templates | [Chapter 9: Templating](09-templating.md) |
| Encrypt secrets | [Chapter 8: Security](08-security.md) |
| Optimize performance | [Performance Tuning](performance-tuning.md) |
| Fix an error | [Troubleshooting](troubleshooting.md) |
| Learn best practices | [Best Practices](best-practices.md) |

## About This Guide

This guide is organized to take you from basic concepts to advanced usage patterns. Each chapter builds on previous knowledge, but chapters can also be read independently as reference material.

### Conventions Used

Throughout this guide, you'll see:

- **Code blocks**: Examples you can copy and use
- **Notes**: Important information highlighted for attention
- **Tips**: Helpful suggestions for better usage
- **Warnings**: Cautions about potential issues

### Example Files

Most examples in this guide reference files that exist in the `examples/` directory of the Rustible repository. You can use these as starting points for your own automation.

## Getting Help

- **GitHub Issues**: [github.com/rustible/rustible/issues](https://github.com/rustible/rustible/issues)
- **Documentation**: This guide and API reference
- **Examples**: Sample playbooks in the repository
