#!/usr/bin/env python3
"""Reproducible LocalNet PoC. No grant submission or shared-network deployment."""
import argparse
import hashlib
import json
import subprocess
import time
from pathlib import Path
from urllib.error import HTTPError

import support as s
import lifecycle
import queries


def current(stage):
    """Check the saved shared revision and its projected state after a real choice."""
    data = s.state()
    return any(row['contractId'] == data['contracts']['shared'] and row['values']['state'] == stage for row in s.pages())


def verify():
    """Compare real transaction identities and values, not screenshots or unscoped global counts."""
    data = s.state()
    expected = set(data['contracts'].values())
    # Expected scopes come from submitted fixtures, independently of the indexer's totals.
    for principal, roles in [('a', ('a', 'shared')), ('b', ('b', 'shared')), ('ab', ('a', 'b', 'shared'))]:
        own = [r for r in s.pages(principal) if r['values']['demoId'].startswith(data['source'])]
        if {r['contractId'] for r in own} != {data['contracts'][role] for role in roles}:
            raise ValueError('Party-scoped projected state differs from expected contract identities')
        for row in own:
            values = row['values']
            if row['contractId'] == data['contracts']['shared']:
                expected_outcome = True if data['stage'] == 'Resolved' else None
                if values['state'] != data['stage'] or values['outcome'] != expected_outcome:
                    raise ValueError('Projected lifecycle state or optional resolution outcome changed')
            # Numeric 10 stays exact: 0.0000000001 × 10000 = 0.0000010000.
            # Int64 max also remains a string, beyond JavaScript's exact integer range.
            if values['quantityLot'] != '9223372036854775807' or values['priceTick'] != '0.0000000001' or values['priceTickBps'] != '0.0000010000':
                raise ValueError('Exact projected values changed')
    events = s.pages(typed=False)
    observed = {(r['kind'], r['contractId']) for r in events}
    if not {tuple(event) for event in data['expectedEvents']}.issubset(observed):
        raise ValueError('Indexer missed a real transaction event')
    # A cursor must not be transferable between otherwise valid API users.
    cursor = s.query('a', limit=1)['nextCursor']
    if not cursor:
        raise ValueError('Expected two visible contracts for cursor control')
    try:
        s.query('b', cursor=cursor)
    except (HTTPError, ValueError):
        pass
    else:
        raise ValueError('Cross-principal cursor was accepted')
    grants_path = s.DIRECTORY / 'grants.json'
    grants = json.loads(grants_path.read_text())
    try:
        denied = json.loads(json.dumps(grants)); denied['principals']['b']['enabled'] = False
        denied['epoch'] = grants['epoch'] + 1
        s.write_private(grants_path, denied)
        try:
            s.query('b')
        except HTTPError as error:
            if error.code != 401:
                raise
        else:
            raise ValueError('Disabled API principal retained access')
    finally:
        # Restore access with a newer epoch; restoring the old file would revive old cursors.
        s.write_private(grants_path, {**grants, 'epoch': grants['epoch'] + 2})
    if not expected.issubset({r['contractId'] for r in s.pages()}):
        raise ValueError('Restored grants did not restore the expected authorized view')
    return len(data['expectedEvents'])


def run():
    """Start from a built DAR and owned infrastructure; retain volumes and generated evidence."""
    print('Preparing demo configuration and deploying the local DAR')
    lifecycle.configure(); lifecycle.deploy()
    data = s.state()
    if data['stage'] in ('Resolved', 'Archived'):
        lifecycle.create(); data = s.state()
    if data['stage'] != 'Draft':
        raise ValueError('Complete the current cycle with transact before repeating acceptance')
    # Complete a bounded snapshot prefix. Exporter/package startup can briefly pause
    # indexing after DAR upload; retry without acknowledging failed pages.
    def snapshot_complete():
        """Retry one source page and inspect its persisted cursor until snapshot paging ends."""
        try:
            s.cli('index', s.DIRECTORY / 'runtime.json', '--once')
            checkpoint = s.metrics()['checkpoint']
            return not json.loads(checkpoint)['snapshot']
        except (ValueError, subprocess.CalledProcessError):
            return False
    s.wait(snapshot_complete, 'Snapshot did not complete; verify current source grants/readiness')
    s.cli('models', 'register', s.DIRECTORY / 'models.json', '--namespace', data['namespace'], '--database-env', 'CANTONGRID_DESTINATION_URL')
    s.cli('models', 'install', data['namespace'], 'markets', 1, '--database-env', 'CANTONGRID_DESTINATION_URL')
    s.cli('models', 'bind', s.DIRECTORY / 'runtime.json', data['namespace'], 'markets', 1)
    queries.seed()
    print('Model installed and bound; starting Rust worker and APIs')
    with s.process('serve'):
        with s.process('index'):
            s.wait(lambda: current('Draft'), 'Draft was not indexed')
            lifecycle.exercise('Publish')
            s.wait(lambda: current('Open'), 'Publish was not indexed')
        before = s.metrics()
        # A real choice occurs while the Rust worker is stopped. Restart must catch it from PQS.
        lifecycle.exercise('Resolve')
        with s.process('index'):
            s.wait(lambda: current('Resolved'), 'Restart missed Resolve')
            event_count = verify()
            query_report = queries.verify(revisions=True, outage=True)
        after = s.metrics()
        if int(after['events']) <= int(before['events']):
            raise ValueError('Offline transaction did not advance durable history')
        # Reopen several times at the same head: no duplicate canonical/model/history effects.
        for _ in range(3):
            s.cli('index', s.DIRECTORY / 'runtime.json', '--once')
        if s.metrics() != after:
            raise ValueError('Restart/re-observation duplicated data or moved the acknowledged cursor')
    # Export assertions and artifact identity, leaving tokens and live contract IDs
    # in private state rather than the report intended for review.
    s.write_private(s.DIRECTORY / 'report.json', {
        'result': 'passed', 'darSha256': s.state()['darSha256'], 'packageId': s.state()['package'],
        'contract': 'Market:PredictionMarket', 'stages': ['Draft', 'Open', 'Resolved'],
        'controlledEventIdentitiesMatched': event_count, 'source': 'authorized node-local PQS',
        'generated': ['PostgreSQL schema', 'TypeScript transport interfaces', 'GraphQL schema'],
        'typedGraphql': True, 'exactValues': True, 'partyPrivacy': True, 'cursorIsolation': True,
        'callerRevocation': True, 'offlineCatchup': True, 'restartWithoutDuplicates': True,
        'history': 'snapshot_forward_partial', 'environment': 'disposable LocalNet',
        'fundedAssetsOrTrades': False, 'modelQueries': query_report})
    print('PASS: real Daml lifecycle -> PQS -> Rust indexer -> exact PostgreSQL projections -> typed GraphQL')
    print('PASS: party privacy, cursor isolation, revocation, offline catch-up and restart without duplicates')
    print('PASS: 54 filtered page sequences, REST/GraphQL parity, revision rollback and exporter recovery')
    print('Evidence: .local/poc/report.json; generated client contracts: .local/poc/generated/')


def main():
    """Dispatch local demo actions; require explicit opt-in before any ledger write."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['run', 'serve', 'transact', 'verify', 'rebuild'])
    parser.add_argument('--allow-ledger-writes', action='store_true')
    parser.add_argument('--choice', choices=['create', 'publish', 'resolve'])
    args = parser.parse_args()
    if args.command in ('run', 'transact', 'rebuild') and not args.allow_ledger_writes:
        parser.error('run/transact/rebuild requires --allow-ledger-writes for the disposable LocalNet')
    if args.command == 'run':
        run()
    elif args.command == 'rebuild':
        import rebuilds
        with s.process('serve'):
            s.wait(lambda: bool(s.query()), 'API did not become ready')
            rebuilds.run()
    elif args.command == 'transact':
        if args.choice == 'create': lifecycle.create()
        elif args.choice in ('publish', 'resolve'): lifecycle.exercise(args.choice.title())
        else: parser.error('transact requires --choice')
        print('Submitted local test choice; the running Rust worker will index its events')
    elif args.command == 'verify':
        verify()
        if s.state().get('queryContracts'): queries.verify()
        print('PASS: current API state/history, model queries and privacy controls')
    else:
        from dashboard import serve
        with s.process('index'), s.process('serve'):
            s.wait(lambda: bool(s.query()), 'API did not become ready')
            serve()


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        # Upstream bodies, command environments and party/contract IDs are private local evidence.
        reason = str(error) if isinstance(error, ValueError) else type(error).__name__
        print(f'PoC failed: {reason}; inspect service readiness and the documented prerequisites')
        raise SystemExit(1) from None
