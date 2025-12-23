#!/usr/bin/python
# -*- coding: utf-8 -*-

# A module that exits with a specific exit code

import json
import os
import sys
import base64

def main():
    args_b64 = os.environ.get('ANSIBLE_MODULE_ARGS', '{}')
    try:
        args = json.loads(base64.b64decode(args_b64).decode('utf-8'))
    except:
        args = json.loads(args_b64) if args_b64 != '{}' else {}

    exit_code = args.get('exit_code', 0)

    result = {
        'changed': False,
        'msg': f'Module exiting with code {exit_code}',
        'exit_code': exit_code,
    }

    print(json.dumps(result))
    sys.exit(exit_code)

if __name__ == '__main__':
    main()
