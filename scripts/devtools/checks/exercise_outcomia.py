"""Existing approved Outcomia LocalNet artifact: protocol→binary creation only, no funding/trading."""
import hashlib
from datetime import datetime,timedelta,timezone
from ..ledger import get,request,token
from .exercise_localnet import BASE

# Exact package previously verified against the deployed DAR; never resolve an alias to newer code.
PACKAGE='73c2c43b962bb68d033a62182bbb76ed27b50375a8b6ded3314220e5d1ba114a'


def bootstrap(writer,parties,identifier):
    """Reuse the deployed package and approved synthetic parties; refuse missing DARs without uploading."""
    if PACKAGE not in get(BASE,'/v2/packages',token())['packageIds']:
        raise ValueError('Previously approved Outcomia package is not deployed; no DAR upload attempted')
    response=request(BASE,'/v2/commands/submit-and-wait-for-transaction',token(writer),body={
        'commands':{'commandId':identifier+'-protocol','userId':writer,'actAs':[parties['operator']],
          'commands':[{'CreateCommand':{'templateId':PACKAGE+':Outcomia.Binary:Protocol','createArguments':{
            'operator':parties['operator'],'protocolName':identifier,'admins':list(parties.values()),
            'nextMarketId':'1','takerFeeBps':'50','feeRevision':'0'}}}]}},timeout=120)
    return next(e['CreatedEvent'] for e in response['transaction']['events'] if 'CreatedEvent' in e)


def actions(parties,identifier):
    """Use the same validated terms shape as the existing local Outcomia lifecycle example."""
    document=b'CantonGrid LocalNet lineage check; no collateral, orders or trading.\n'
    terms={'question':'LocalNet lineage check','collateral':{'admin':parties['operator'],'id':'LOCAL-TEST-ONLY'},
        'rules':{'yesCriteria':'The fixture completes.','noCriteria':'The fixture fails.',
            'invalidCriteria':'The fixture is interrupted.','evidenceRequirements':'Local test report.',
            'documentUri':'https://example.invalid/cantongrid-lineage',
            'documentSha256':hashlib.sha256(document).hexdigest()},
        'closeAt':(datetime.now(timezone.utc)+timedelta(days=1)).isoformat().replace('+00:00','Z'),
        'earliestResolutionAt':None,'fallback':None,'resolutionDelaySeconds':'86400',
        'priceTick':'0.0000000001','quantityLot':'9223372036854775807'}
    return [('Protocol_CreateBinary',{'creator':parties['operator'],'terms':terms})]
