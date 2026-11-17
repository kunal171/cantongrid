#!/usr/bin/env python3
"""Validate an existing DAR on LocalNet without uploading or vetting it."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
from urllib.error import HTTPError, URLError
from localnet_api import request, token


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('dar', type=Path)
    args = parser.parse_args()
    payload = args.dar.read_bytes()
    digest = hashlib.sha256(payload).hexdigest()
    request('http://127.0.0.1:3975', '/v2/dars/validate', token(), body=payload,
            content_type='application/octet-stream', read_json=False, timeout=120)
    print(json.dumps({'dar': args.dar.name, 'sha256': digest,
                      'validated': True, 'uploaded': False}, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, HTTPError, URLError) as exc:
        print(f'Local DAR validation failed: {exc}', file=sys.stderr)
        sys.exit(1)
