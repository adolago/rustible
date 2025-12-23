#!/usr/bin/env python3
"""
Dynamic Inventory Script Compatibility Test

This script demonstrates dynamic inventory format that Rustible should support.
When called with --list, it outputs JSON inventory data.
When called with --host <hostname>, it outputs host-specific variables.

Tests: JSON output format, _meta/hostvars, groups, children
"""

import json
import sys
import argparse


def get_inventory():
    """Return the complete inventory as a dictionary."""
    return {
        "webservers": {
            "hosts": ["web1", "web2", "web3"],
            "vars": {
                "nginx_version": "1.18",
                "server_type": "nginx",
                "max_connections": 1000
            }
        },
        "databases": {
            "hosts": ["db1", "db2"],
            "vars": {
                "db_engine": "postgresql",
                "db_version": "14"
            }
        },
        "appservers": {
            "hosts": ["app1", "app2", "app3"],
            "vars": {
                "app_user": "apprunner",
                "app_environment": "production"
            }
        },
        "production": {
            "children": ["webservers", "databases", "appservers"],
            "vars": {
                "environment": "production",
                "log_level": "warn"
            }
        },
        "staging": {
            "hosts": ["staging1"],
            "vars": {
                "environment": "staging",
                "log_level": "debug"
            }
        },
        "_meta": {
            "hostvars": {
                "web1": {
                    "ansible_host": "192.168.1.10",
                    "ansible_user": "deploy",
                    "http_port": 80
                },
                "web2": {
                    "ansible_host": "192.168.1.11",
                    "ansible_user": "deploy",
                    "http_port": 8080
                },
                "web3": {
                    "ansible_host": "192.168.1.12",
                    "ansible_user": "webadmin",
                    "ansible_ssh_private_key_file": "~/.ssh/web_key"
                },
                "db1": {
                    "ansible_host": "192.168.1.20",
                    "ansible_user": "dbadmin",
                    "db_port": 5432,
                    "db_role": "primary"
                },
                "db2": {
                    "ansible_host": "192.168.1.21",
                    "ansible_user": "dbadmin",
                    "db_port": 5432,
                    "db_role": "replica"
                },
                "app1": {
                    "ansible_host": "192.168.1.30",
                    "app_port": 5000
                },
                "app2": {
                    "ansible_host": "192.168.1.31",
                    "app_port": 5001
                },
                "app3": {
                    "ansible_host": "192.168.1.32",
                    "app_port": 5002
                },
                "staging1": {
                    "ansible_host": "192.168.2.10",
                    "environment": "staging"
                }
            }
        }
    }


def get_host_vars(hostname):
    """Return variables for a specific host."""
    inventory = get_inventory()
    hostvars = inventory.get("_meta", {}).get("hostvars", {})
    return hostvars.get(hostname, {})


def main():
    parser = argparse.ArgumentParser(description='Dynamic inventory script')
    parser.add_argument('--list', action='store_true',
                        help='List all groups and hosts')
    parser.add_argument('--host', type=str,
                        help='Get variables for a specific host')
    args = parser.parse_args()

    if args.list:
        print(json.dumps(get_inventory(), indent=2))
    elif args.host:
        print(json.dumps(get_host_vars(args.host), indent=2))
    else:
        # Default to --list
        print(json.dumps(get_inventory(), indent=2))


if __name__ == '__main__':
    main()
