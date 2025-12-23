#!/usr/bin/python
# -*- coding: utf-8 -*-

# A module that writes to both stdout and stderr

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

    stderr_msg = args.get('stderr_msg', 'Warning: some stderr output')

    # Write to stderr
    print(stderr_msg, file=sys.stderr)

    result = {
        'changed': True,
        'msg': 'Module executed with stderr output',
        'has_stderr': True,
    }

    print(json.dumps(result))

if __name__ == '__main__':
    main()
