#!/usr/bin/python
# -*- coding: utf-8 -*-

# A simple test module that returns a basic result
# This module reads arguments from ANSIBLE_MODULE_ARGS environment variable

import json
import os
import base64

def main():
    args_b64 = os.environ.get('ANSIBLE_MODULE_ARGS', '{}')
    try:
        args = json.loads(base64.b64decode(args_b64).decode('utf-8'))
    except:
        args = json.loads(args_b64) if args_b64 != '{}' else {}

    name = args.get('name', 'default')
    state = args.get('state', 'present')

    result = {
        'changed': True,
        'msg': f'Simple module executed with name={name}, state={state}',
        'name': name,
        'state': state,
    }

    print(json.dumps(result))

if __name__ == '__main__':
    main()
