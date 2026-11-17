"""Validate development log retention without starting containers or pulling images."""
import json
import subprocess

from ..common import ENV, ROOT


def validate(config):
    """Reject uncapped logs for every service, including inactive profiles."""
    for name, service in config['services'].items():
        logging = service.get('logging', {})
        options = logging.get('options', {})
        if (logging.get('driver') != 'json-file'
                or options.get('max-size') != '10m'
                or str(options.get('max-file')) != '3'):
            raise ValueError(f'{name}: expected json-file rotation at 10m x 3 files')


def main():
    """Check tracked fixtures; the LocalNet renderer is tested without a checkout."""
    services = 0
    for filename in ('pqs', 'visibility', 'storage'):
        config = json.loads(subprocess.check_output([
            'docker', 'compose', '-f', str(ROOT / f'dev/{filename}.compose.yaml'),
            'config', '--format', 'json',
        ], env=ENV))
        validate(config)
        services += len(config['services'])
    print(f'PASS: {services} fixture services have bounded Docker log retention')
