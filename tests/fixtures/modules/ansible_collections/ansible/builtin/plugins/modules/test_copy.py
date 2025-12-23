#!/usr/bin/python
# -*- coding: utf-8 -*-

# Mock ansible.builtin.copy module for testing FQCN resolution

import json
import os
import base64

def main():
    args_b64 = os.environ.get('ANSIBLE_MODULE_ARGS', '{}')
    try:
        args = json.loads(base64.b64decode(args_b64).decode('utf-8'))
    except:
        args = json.loads(args_b64) if args_b64 != '{}' else {}

    src = args.get('src', '')
    dest = args.get('dest', '')
    content = args.get('content', '')

    result = {
        'changed': True,
        'msg': f'Mock copy from {src} to {dest}',
        'dest': dest,
        'src': src,
        'fqcn': 'ansible.builtin.test_copy',
    }

    print(json.dumps(result))

if __name__ == '__main__':
    main()
