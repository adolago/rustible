#!/usr/bin/python
# -*- coding: utf-8 -*-

# Another custom collection module for testing

import json
import os
import base64

def main():
    args_b64 = os.environ.get('ANSIBLE_MODULE_ARGS', '{}')
    try:
        args = json.loads(base64.b64decode(args_b64).decode('utf-8'))
    except:
        args = json.loads(args_b64) if args_b64 != '{}' else {}

    result = {
        'changed': False,
        'msg': 'Nested module in custom collection executed',
        'fqcn': 'custom.test_collection.nested_module',
    }

    print(json.dumps(result))

if __name__ == '__main__':
    main()
