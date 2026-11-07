import { createMemo } from "solid-js";
import { render } from "@solidjs/web";

// R2h (patterns/flip/sync-memo-jsx-read): `sync: true` makes an async compute
// an error (dev, SYNC_NODE_RECEIVED_ASYNC) or an unawaited value (production),
// never a pending source, while a pending read inside a sync compute still
// suspends it. The boundary claim is not proved: uncertifiable, never SC5003.
function SyncAsyncRead() {
  const memo = createMemo(async () => 1, { sync: true });
  return <i>{String(memo())}</i>;
}

// A `sync` value the analysis cannot read: the same.
function SyncFlagRead(props: { flag: boolean }) {
  const memo = createMemo(async () => 1, { sync: props.flag });
  return <i>{String(memo())}</i>;
}

// `sync: false` is the plain async node: the boundary claim stands.
function SyncFalseRead() {
  const memo = createMemo(async () => 1, { sync: false });
  return <i>{String(memo())}</i>;
}

render(() => <SyncAsyncRead />, document.body);
render(() => <SyncFlagRead flag />, document.body);
render(() => <SyncFalseRead />, document.body);
