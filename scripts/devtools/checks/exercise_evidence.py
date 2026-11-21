"""Pure M0 comparison of authorized Ledger Effects and PQS evidence; no ancestry inference."""
import json


def canonical(value):
    """Preserve exact JSON values while making dictionary ordering irrelevant to comparison."""
    return json.dumps(value, sort_keys=True, separators=(',', ':'))


def compare_view(ledger, pqs):
    """Compare source receipts, not final ACS counts. A missing/extra event fails the experiment.

    Exercise and create witnesses are checked independently. Archive receipts are
    compared with consuming exercises for reconciliation research only: this does
    not choose authoritative runtime ownership of the lifecycle transition.
    """
    expected = {'creates': {}, 'exercises': {}, 'archives': {}}
    observed = {'creates': {}, 'exercises': {}, 'archives': {}}

    def insert(collection, key, value):
        """Duplicate event IDs must fail instead of being hidden by dictionary replacement."""
        if key in collection:
            raise ValueError('Duplicate receipt in source evidence')
        collection[key] = value

    for response in ledger.values():
        if response is None:
            continue
        for tagged in response['transaction']['events']:
            if len(tagged) != 1:
                raise ValueError('Unexpected Ledger event variant')
            kind, event = next(iter(tagged.items()))
            key = (int(event['offset']), event['nodeId'])
            base = [event['contractId'], event['templateId'], sorted(event['witnessParties'])]
            if kind == 'CreatedEvent':
                insert(expected['creates'], key, base + [event['createArgument'],
                       sorted(event['signatories']), sorted(event['observers'])])
            elif kind == 'ExercisedEvent':
                insert(expected['exercises'], key, base + [event['choice'], event['consuming'],
                       event['choiceArgument'], event.get('exerciseResult'),
                       sorted(event['actingParties']), event['lastDescendantNodeId']])
                if event['consuming']:
                    insert(expected['archives'], key, event['contractId'])
            else:
                raise ValueError('Unsupported Ledger event shape in exercise evidence')
    for kind, rows in pqs.items():
        if kind not in observed or not isinstance(rows, list):
            raise ValueError('Unsupported PQS evidence collection')
        for row in rows:
            if row.get('redaction_id') is not None:
                raise ValueError('Redacted evidence needs a separate supported comparison policy')
            field = {'creates': 'create_event_id', 'exercises': 'exercise_event_id',
                     'archives': 'archive_event_id'}[kind]
            identity = row[field]
            key = (int(identity['offset']), identity['node_id'])
            # PQS's FQN starts with package *name*; Ledger template IDs start
            # with package *hash*. Preserve the separate package_id to compare
            # concrete versions; matching a package-name alias alone is unsafe.
            parts = row['template_fqn'].split(':')
            if len(parts) != 3 or parts[0] != row['package_name'] or not row['package_id']:
                raise ValueError('PQS template identity lacks unambiguous package evidence')
            template = ':'.join([row['package_id'], *parts[1:]])
            base = [row['contract_id'], template, sorted(row['witnesses'])]
            if kind == 'creates':
                value = base + [row['payload'], sorted(row['signatories']), sorted(row['observers'])]
            elif kind == 'exercises':
                value = base + [row['choice'], row['consuming'], row['argument'], row['result'],
                                sorted(row['controllers']), row['last_descendant_node_id']]
            else:
                value = row['contract_id']
            insert(observed[kind], key, value)
    result = {}
    for kind in expected:
        left, right = expected[kind], observed[kind]
        result[kind] = {'ledger': len(left), 'pqs': len(right),
                        'identities_match': left.keys() == right.keys(),
                        'values_match': left.keys() == right.keys() and all(
                            canonical(left[key]) == canonical(right[key]) for key in left)}
    return result
