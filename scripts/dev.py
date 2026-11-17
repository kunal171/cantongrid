#!/usr/bin/env python3
"""LocalNet research tools. Rust's cantongrid executable is the product CLI."""
import argparse
import importlib
import subprocess
import sys
from urllib.error import HTTPError

ROUTES = {
    'localnet': 'localnet',
    'pqs': 'pqs',
    'validate-dar': 'validate_dar',
    'visibility': 'setup_visibility',
    'check': None,
}
CHECKS = {name.replace('_', '-'): name for name in ('ledger', 'pqs', 'reader', 'visibility', 'visibility_pqs')}


def main():
    parser = argparse.ArgumentParser(description=__doc__, epilog=(
        'localnet prepare|up|status|stop|logs|images; pqs up|stop|status|reader [--visibility]; '
        'validate-dar <dar>; visibility setup; check ledger|pqs|reader|visibility|visibility-pqs'))
    parser.add_argument('command', choices=ROUTES)
    # Leave subcommand arguments (including --help) to their own parser.
    if len(sys.argv) < 2 or sys.argv[1] in ('-h', '--help'):
        parser.parse_args()
        return
    args = parser.parse_args(sys.argv[1:2])
    rest = sys.argv[2:]
    if args.command == 'check':
        if not rest or rest[0] not in CHECKS:
            parser.error('check requires: ' + ', '.join(CHECKS))
        name = rest.pop(0)
        module = importlib.import_module('devtools.checks.' + CHECKS[name])
        if name == 'pqs':
            module.main(rest)
        else:
            check_parser = argparse.ArgumentParser(prog=f'{parser.prog} check {name}', description=module.__doc__)
            check_parser.parse_args(rest)
            module.main()
    else:
        module = importlib.import_module('devtools.' + ROUTES[args.command])
        if args.command == 'visibility':
            setup_parser = argparse.ArgumentParser(prog=f'{parser.prog} visibility', description=module.__doc__)
            setup_parser.add_argument('action', choices=['setup'])
            setup_parser.parse_args(rest)
            module.main()
        else:
            module.main(rest)


if __name__ == '__main__':
    try:
        main()
    except ValueError as exc:
        print(f'Development helper failed: {exc}', file=sys.stderr)
        sys.exit(1)
    except HTTPError as exc:
        print(f'Development helper HTTP request failed (status {exc.code})', file=sys.stderr)
        sys.exit(1)
    except (OSError, KeyError, subprocess.CalledProcessError) as exc:
        # Avoid exposing bodies, tokens or private identifiers from upstream diagnostics.
        print(f'Development helper failed ({type(exc).__name__}); inspect local service health/configuration', file=sys.stderr)
        sys.exit(1)
