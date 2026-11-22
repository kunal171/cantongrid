"""Read-only API matrix over the worker's already approved LocalNet controls."""
import json
import secrets
import socket
import subprocess
import time
from urllib.error import HTTPError
from urllib.parse import quote
from urllib.request import Request

from ..common import write_private
from ..http import open_local
from .pipeline import application_token


def verify_graph(directory,config,env,binary,parties,scope,root,*,outcomia=False):
    """Use independent organization grants; no Ledger rights, parties or contracts are changed here."""
    with socket.socket() as socket_probe:
        socket_probe.bind(('127.0.0.1',0))
        port=socket_probe.getsockname()[1]
    key=secrets.token_hex(32)
    env={**env,'CANTONGRID_M1_API_KEY':key,'CANTONGRID_M1_CURSOR_KEY':secrets.token_hex(32)}
    config['api'].update(bind=f'127.0.0.1:{port}',issuer='cantongrid-local-check',audience='cantongrid-api-check',grants_file='grants.json')
    selected={'operator':[parties['operator']]} if scope=='operator' else {'a':[parties['a']],'b':[parties['b']],'ab':[parties['a'],parties['b']]}
    grants={'version':1,'epoch':1,'principals':{name:{'enabled':True,'parties':values} for name,values in selected.items()}}
    write_private(directory/'grants.json',grants);write_private(directory/'runtime.json',config)
    process=subprocess.Popen([binary,'serve',str(directory/'runtime.json')],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    base=f'http://127.0.0.1:{port}'

    def query(path,subject,body=None):
        """Capture only status/JSON; never follow redirects or print bearer/private node identities."""
        headers={'Authorization':'Bearer '+application_token(subject,key)}
        if body is not None:headers['Content-Type']='application/json'
        request=Request(base+path,headers=headers,data=None if body is None else json.dumps(body).encode())
        try:
            with open_local(request,timeout=10) as response:
                if response.headers.get('Cache-Control')!='no-store':raise ValueError('Lineage response permits caching')
                return response.status,json.load(response)
        except HTTPError as error:
            return error.code,json.loads(error.read())
    try:
        deadline=time.monotonic()+10
        while True:
            try:
                with socket.create_connection(('127.0.0.1',port),timeout=1):break
            except OSError:
                if process.poll() is not None or time.monotonic()>deadline:raise ValueError('Lineage API did not start') from None
                time.sleep(0.1)
        path=f'/v1/contracts/{quote(root,safe="")}/lineage?direction=children&limit=1'
        totals={}
        for subject in selected:
            status,result=query(path,subject)
            expected=(2 if scope=='operator' else 1) if outcomia else (3 if scope=='operator' else 1)
            if status!=200 or result['visibleTotal']!=str(expected) or result['provenance']['complete'] is not False:
                raise ValueError('Lineage API did not match the controlled visible graph')
            items=list(result['items']);cursor=result['nextCursor']
            for _ in range(4):
                if cursor is None:break
                status,result=query(path+'&cursor='+quote(cursor,safe=''),subject)
                if status!=200:raise ValueError('Lineage resume failed')
                items.extend(result['items']);cursor=result['nextCursor']
            if cursor is not None or len(items)!=expected:raise ValueError('Lineage page count differs from visible total')
            totals[subject]=len(items)
            graph_query='''
                query Lineage($root: String!) {
                    contractLineage(contractId: $root, direction: "children", limit: 1) {
                        visibleTotal
                        items
                        provenance
                    }
                }
            '''
            status,graph=query('/graphql',subject,{'query':graph_query,'variables':{'root':root}})
            if status!=200 or graph.get('errors') or graph['data']['contractLineage']['visibleTotal']!=str(expected):
                raise ValueError('REST and GraphQL lineage differ')
        subject=next(iter(selected))
        if query(path,'ungranted')[0]!=401:raise ValueError('Ungrant principal accessed lineage')
        if scope=='ab':
            pair=json.loads((directory/('Protocol_CreateBinary.json' if outcomia else 'Factory_Pair.json')).read_text())
            children=[e['CreatedEvent'] for e in pair['transaction']['events'] if 'CreatedEvent' in e]
            right=next(e['contractId'] for e in children if e['templateId'].endswith(':BinaryMarket')) if outcomia else next(e['contractId'] for e in children if e['createArgument'].get('label')=='right')
            hidden=query(f'/v1/contracts/{quote(right,safe="")}/lineage?direction=parents','a')
            unknown=query('/v1/contracts/unknown/lineage?direction=parents','a')
            if hidden!=unknown or hidden[0]!=404:raise ValueError('Hidden root differs from an unknown root')
        grants['epoch']=2;grants['principals'][subject]['enabled']=False
        write_private(directory/'grants.json',grants)
        if query(path,subject)[0]!=401:raise ValueError('Caller revocation did not stop lineage delivery')
        return {'source_scope':scope,'visible_edges':totals,'rest_graphql_match':True,'ungranted_denied':True,'caller_revocation_denied':True,'one_hop_only':True}
    finally:
        process.terminate()
        try:process.wait(timeout=5)
        except subprocess.TimeoutExpired:process.kill();process.wait(timeout=5)
