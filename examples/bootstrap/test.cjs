/** Offline behavior checks: baseline publication precedes events, failed effects never advance ACKs. */
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { resolve } = require('node:path');
const build = process.env.CANTONGRID_EXAMPLE_BUILD;
if (!build) throw new Error('Set CANTONGRID_EXAMPLE_BUILD to the compiled example directory');
const { BootstrapApi, ApiError } = require(resolve(build, 'client.js'));
const { installBaseline, consumePage } = require(resolve(build, 'consumer.js'));
const id = '11111111-1111-1111-1111-111111111111';
const exact = '12345678901234567890.0000000001';
const contract = (contractId) => ({ contractId, payload: { type: 'raw_json', value: `{"amount":"${exact}"}` } });

test('stage full baseline before events; ACK only committed effects; retry unacknowledged work', async () => {
  const calls = [], order = [];
  let fail = true;
  const originalFetch = global.fetch;
  global.fetch = async (url, options) => {
    calls.push([url.pathname, options.method, options.body]);
    assert.equal(options.redirect, 'error');
    assert.equal(options.headers.Authorization, 'Bearer local-fixture-only');
    let response;
    if (url.pathname.endsWith('/bootstrap')) response = { id, formatVersion: 1, atomicHandoff: true };
    else if (url.pathname.endsWith('/snapshot')) response = {
      formatVersion: 1, items: [contract(url.searchParams.has('cursor') ? 'z' : 'a')],
      nextCursor: url.searchParams.has('cursor') ? null : 'page-two',
    };
    else if (url.pathname.endsWith('/events')) response = { formatVersion: 1, events: [{ eventId: 'create-new' }], nextCursor: 'receipt', more: false };
    else { order.push('ack'); response = { ok: true }; }
    return Response.json(response);
  };
  const sink = {
    async beginBaseline() { order.push('begin'); },
    async stage(rows) { assert.equal(JSON.parse(rows[0].payload.value).amount, exact); order.push(rows[0].contractId); },
    async commitBaseline() { order.push('publish'); },
    async abortBaseline() { order.push('abort'); },
    async applyPage() { order.push('apply'); if (fail) throw new Error('transaction failed'); },
  };
  try {
    const api = new BootstrapApi('http://127.0.0.1:8080', async () => 'local-fixture-only');
    assert.equal(await installBaseline(api, sink), id);
    assert.deepEqual(order, ['begin', 'a', 'z', 'publish']);
    await assert.rejects(consumePage(api, sink, id), /transaction failed/);
    assert.equal(calls.filter(c => c[0].endsWith('/acknowledge')).length, 0);
    fail = false;
    await consumePage(api, sink, id);
    assert.deepEqual(order.slice(-3), ['apply', 'apply', 'ack']);
  } finally { global.fetch = originalFetch; }
});

test('a baseline coverage gap aborts staging without publication or implicit reset', async () => {
  const order = [], originalFetch = global.fetch;
  global.fetch = async url => url.pathname.endsWith('/bootstrap')
    ? Response.json({ id, formatVersion: 1, atomicHandoff: true })
    : Response.json({ coverage: 'gap' }, { status: 409 });
  try {
    const api = new BootstrapApi('http://127.0.0.1:8080', async () => 'local-fixture-only');
    const sink = {
      async beginBaseline() { order.push('begin'); }, async stage() { order.push('stage'); },
      async commitBaseline() { order.push('publish'); }, async abortBaseline() { order.push('abort'); },
      async applyPage() { order.push('apply'); },
    };
    await assert.rejects(installBaseline(api, sink), e => e instanceof ApiError && e.status === 409 && e.coverage === 'gap');
    assert.deepEqual(order, ['begin', 'abort']);
  } finally { global.fetch = originalFetch; }
});
