"""Separate disposable PostgreSQL destination for Rust ingestion tests."""
import argparse
import subprocess
from .common import ROOT


def main(argv=None):
    """Manage only the separate owned destination; stop removes containers but keeps its volume."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['up', 'status', 'stop'])
    args = parser.parse_args(argv)
    commands = {'up': ['up', '-d', '--wait'], 'status': ['ps', '-a'], 'stop': ['down']}
    subprocess.run(['docker', 'compose', '--progress', 'plain', '-p', 'cantongrid-storage',
                    '-f', str(ROOT / 'dev/storage.compose.yaml'), *commands[args.command]], check=True)
