#!/usr/bin/python
# -*- coding: utf-8 -*-

# An echo module that returns the arguments passed to it
# Useful for testing argument passing

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
        'msg': 'Arguments echoed successfully',
        'args': args,
    }

    print(json.dumps(result))

if __name__ == '__main__':
    main()
