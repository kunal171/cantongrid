# Bootstrap consumer example

Use the alpha.7 API and owned migration 012. See the
[bootstrap guide](../../docs/guide/bootstrap.md) for the protocol and limits.
`client.ts` contains transport/endpoint methods; `consumer.ts` contains the
baseline-to-feed sequence. No npm runtime packages are required.

```typescript
import { BootstrapApi } from "./client";
import { installBaseline, consumePage, Sink } from "./consumer";

async function run(token: () => Promise<string>, sink: Sink, signal: AbortSignal) {
  const api = new BootstrapApi("http://127.0.0.1:8080", token);
  const subscription = await installBaseline(api, sink, undefined, signal);
  while (!signal.aborted) {
    await consumePage(api, sink, subscription, signal);
  }
}
```

Implement `Sink` with your application's durable storage. Stage all baseline rows
before publication, publish the view and subscription ID together, and make each
event-page application transactional and idempotent. Configure a stable source
generation for deduplication; a signed cursor is not a deduplication key. On restart,
load the stored subscription instead of blindly registering again. A failed page
is never ACKed. A 409 gap or unavailable coverage needs an explicit resynchronization;
the example does not reset state automatically. Retrying a fresh bootstrap name
is safe only within the same live, unconsumed registration lifetime.

Keep financial values as strings. Raw JSON payload text can be decoded for fields
whose declared types are strings; do not turn amounts or large integers into JS
numbers. The client is a small example, not a complete generated/schema-validating SDK.
Supply request deadlines through `AbortSignal` and keep credentials out of logs.

Optional example checks use an installed TypeScript compiler and Node 22+:

```bash
mkdir -p .local/bootstrap-example
tsc --strict --target ES2022 --module commonjs --lib ES2022,DOM \
  --outDir .local/bootstrap-example \
  examples/bootstrap/client.ts examples/bootstrap/consumer.ts
CANTONGRID_EXAMPLE_BUILD="$PWD/.local/bootstrap-example" \
  node --test examples/bootstrap/test.cjs
```

The offline checks prove full staging precedes publication, exact amount strings
survive transport, failed processing sends no ACK, a coverage gap aborts staging, and an incomplete baseline is never published.
Rust storage/API tests cover the server behavior. The
[operations helper](../operations/README.md) checks the retained LocalNet fixture
without submitting ledger transactions.
