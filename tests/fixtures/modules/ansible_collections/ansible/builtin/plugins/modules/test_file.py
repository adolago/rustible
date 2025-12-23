#!/usr/bin/python
# -*- coding: utf-8 -*-

# Mock ansible.builtin.file module for testing FQCN resolution

import json
import os
import base64

def main():
    args_b64 = os.environ.get('ANSIBLE_MODULE_ARGS', '{}')
    try:
        args = json.loads(base64.b64decode(args_b64).decode('utf-8'))
    except:
        args = json.loads(args_b64) if args_b64 != '{}' else {}

    path = args.get('path', '')
    state = args.get('state', 'file')

    result = {
        'changed': True,
        'msg': f'Mock file operation on {path} with state {state}',
        'path': path,
        'state': state,
        'fqcn': 'ansible.builtin.test_file',
    }

    print(json.dumps(result))

if __name__ == '__main__':
    main()
