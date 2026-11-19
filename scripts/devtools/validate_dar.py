"""Validate an existing DAR on LocalNet without uploading or vetting it."""
import argparse
import hashlib
import json
from pathlib import Path
from .ledger import request, token


def main(argv=None):
    """Validate DAR bytes through LocalNet without uploading or submitting contract commands."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('dar', type=Path)
    args = parser.parse_args(argv)
    payload = args.dar.read_bytes()
    digest = hashlib.sha256(payload).hexdigest()
    request('http://127.0.0.1:3975', '/v2/dars/validate', token(), body=payload,
            content_type='application/octet-stream', read_json=False, timeout=120)
    print(json.dumps({'dar': args.dar.name, 'sha256': digest,
                      'validated': True, 'uploaded': False}, indent=2))
