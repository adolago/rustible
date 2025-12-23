#!/usr/bin/python
# -*- coding: utf-8 -*-

# A module that always fails - for testing error handling

import json
import os
import base64

def main():
    args_b64 = os.environ.get('ANSIBLE_MODULE_ARGS', '{}')
    try:
        args = json.loads(base64.b64decode(args_b64).decode('utf-8'))
    except:
        args = json.loads(args_b64) if args_b64 != '{}' else {}

    error_message = args.get('msg', 'Module failed as expected')

    result = {
        'failed': True,
        'msg': error_message,
        'changed': False,
    }

    print(json.dumps(result))

if __name__ == '__main__':
    main()
