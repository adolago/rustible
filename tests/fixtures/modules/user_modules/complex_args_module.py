#!/usr/bin/python
# -*- coding: utf-8 -*-

# A module that handles complex argument types for testing

import json
import os
import base64

def main():
    args_b64 = os.environ.get('ANSIBLE_MODULE_ARGS', '{}')
    try:
        args = json.loads(base64.b64decode(args_b64).decode('utf-8'))
    except:
        args = json.loads(args_b64) if args_b64 != '{}' else {}

    # Extract various argument types
    string_arg = args.get('string_arg', '')
    int_arg = args.get('int_arg', 0)
    bool_arg = args.get('bool_arg', False)
    list_arg = args.get('list_arg', [])
    dict_arg = args.get('dict_arg', {})
    nested_arg = args.get('nested_arg', {})

    result = {
        'changed': True,
        'msg': 'Complex arguments processed successfully',
        'received': {
            'string_arg': string_arg,
            'string_arg_type': type(string_arg).__name__,
            'int_arg': int_arg,
            'int_arg_type': type(int_arg).__name__,
            'bool_arg': bool_arg,
            'bool_arg_type': type(bool_arg).__name__,
            'list_arg': list_arg,
            'list_arg_type': type(list_arg).__name__,
            'dict_arg': dict_arg,
            'dict_arg_type': type(dict_arg).__name__,
            'nested_arg': nested_arg,
        }
    }

    print(json.dumps(result))

if __name__ == '__main__':
    main()
