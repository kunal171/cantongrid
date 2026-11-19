"""Run the pinned upstream LocalNet; this is a dev helper, not the CantonGrid CLI."""
import argparse
import json
import os
import subprocess

from .common import ROOT, write_private
CHECKOUT = ROOT / '.local/cn-quickstart'
REVISION = '8b898ed3ccb15e243b22ee8d0ed3573fb91ee6b9'
PROJECT = 'cantongrid-localnet'
PROFILES = ['--profile', 'sv', '--profile', 'app-provider', '--profile', 'app-user']
CONFIG = ROOT / '.local/localnet.compose.json'


def run(args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def prepare():
    CHECKOUT.parent.mkdir(parents=True, exist_ok=True)
    if not CHECKOUT.exists():
        run(['git', 'clone', '--no-checkout', 'https://github.com/digital-asset/cn-quickstart.git', str(CHECKOUT)])
        run(['git', '-C', str(CHECKOUT), 'checkout', '--detach', REVISION])
    actual = subprocess.check_output(['git', '-C', str(CHECKOUT), 'rev-parse', 'HEAD'], text=True).strip()
    if actual != REVISION:
        raise ValueError('Upstream checkout differs from the pinned revision; review it before using this helper.')
    if subprocess.check_output(['git', '-C', str(CHECKOUT), 'status', '--porcelain'], text=True).strip():
        raise ValueError('Upstream checkout has local changes; preserve/review them before running LocalNet.')
    return CHECKOUT / 'quickstart'


def apply_local_defaults(config):
    """Bound log retention without changing upstream storage or resource limits."""
    for name, service in config['services'].items():
        service['container_name'] = f'{PROJECT}-{name}'
        # 10 MB x 3 files bounds retained stdout/stderr to about 30 MB per service.
        service['logging'] = {
            'driver': 'json-file',
            'options': {'max-size': '10m', 'max-file': '3'},
        }
        if name in {'canton', 'splice'}:
            # Both pinned image entrypoints otherwise default these levels to DEBUG.
            service.setdefault('environment', {}).update(
                LOG_LEVEL_STDOUT='INFO', LOG_LEVEL_CANTON='INFO')
        for port in service.get('ports', []):
            port['host_ip'] = '127.0.0.1'


def render():
    quickstart = prepare()
    module = quickstart / 'docker/modules/localnet'
    pins = dict(line.split('=', 1) for line in (quickstart / '.env').read_text().splitlines()
                if line and not line.startswith('#') and '=' in line)
    host_keys = {'PATH', 'HOME', 'USER', 'LANG', 'TMPDIR', 'XDG_RUNTIME_DIR',
                 'SSL_CERT_FILE', 'SSL_CERT_DIR'}
    env = {key: value for key, value in os.environ.items()
           if key in host_keys or key.startswith('DOCKER_')
           or key.lower() in {'http_proxy', 'https_proxy', 'all_proxy', 'no_proxy'}}
    # Explicit disposable LocalNet settings, independent of shell deployment variables.
    env.pop('TEST_PORT', None)
    env.update(LOCALNET_DIR=str(module), LOCALNET_ENV_DIR=str(module / 'env'),
               IMAGE_TAG=pins['SPLICE_VERSION'],
               IMAGE_REPO='ghcr.io/digital-asset/decentralized-canton-sync/docker/',
               DOCKER_NETWORK=PROJECT, PARTY_HINT='cantongrid-localparty-1',
               DB_PORT='5440', DB_SERVER='postgres',
               DB_USER='cnadmin', DB_PASSWORD='supersafe',
               APP_USER_PROFILE='on', APP_PROVIDER_PROFILE='on', SV_PROFILE='on',
               APP_USER_UI_PORT='12000', APP_PROVIDER_UI_PORT='13000', SV_UI_PORT='14000',
               AUTH_APP_PROVIDER_AUDIENCE='https://canton.network.global',
               AUTH_APP_USER_AUDIENCE='https://canton.network.global')
    base = ['docker', 'compose', '--progress', 'plain', '-p', PROJECT,
            '--env-file', str(module / 'compose.env'),
            '--env-file', str(module / 'env/common.env'),
            '-f', str(module / 'compose.yaml'),
            '-f', str(module / 'resource-constraints.yaml'), *PROFILES]
    config = json.loads(subprocess.check_output([*base, 'config', '--format', 'json'], env=env))
    apply_local_defaults(config)
    write_private(CONFIG, config)
    return ['docker', 'compose', '--progress', 'plain', '-p', PROJECT, '-f', str(CONFIG), *PROFILES]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['prepare', 'up', 'status', 'stop', 'logs', 'images'])
    args = parser.parse_args(argv)
    if args.command == 'prepare':
        prepare()
        print(f'Prepared upstream revision {REVISION}')
        return
    compose = render()
    commands = {
        'up': ['up', '-d'],
        'status': ['ps', '-a'],
        'stop': ['down'],  # Keep named volumes; no destructive cleanup command.
        'logs': ['logs', '--tail', '80', 'canton', 'splice'],
        'images': ['images'],
    }
    run([*compose, *commands[args.command]])
