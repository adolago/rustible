#!/usr/bin/python
# -*- coding: utf-8 -*-

# Mock custom collection module for testing custom namespace.collection.module resolution

import json
import os
import base64

def main():
    args_b64 = os.environ.get('ANSIBLE_MODULE_ARGS', '{}')
    try:
        args = json.loads(base64.b64decode(args_b64).decode('utf-8'))
    except:
        args = json.loads(args_b64) if args_b64 != '{}' else {}

    custom_arg = args.get('custom_arg', 'default_value')

    result = {
        'changed': True,
        'msg': f'Custom collection module executed with custom_arg={custom_arg}',
        'custom_arg': custom_arg,
        'namespace': 'custom',
        'collection': 'test_collection',
        'fqcn': 'custom.test_collection.custom_module',
    }

    print(json.dumps(result))

if __name__ == '__main__':
    main()
