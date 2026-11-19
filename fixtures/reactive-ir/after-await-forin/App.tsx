import { createMemo, createRoot, createSignal } from "solid-js";

const [source] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// ADR 1117 (`js-forin`): a `for…in` an async memo runs after its `await` walks a Proxy's keys, so its `ownKeys` trap reads with no call written at the loop.
// Each definition is written on one line.
const keyed: object = new Proxy({}, { ownKeys: () => { source(); return []; } });

createRoot(() => {
  // A row at `keyed`: the loop runs after the await.
  createMemo(async () => { await pause(); for (const k in keyed) void k; return 1; });
  // No row: the same loop before the await, a tracked read of the memo's own.
  createMemo(async () => { for (const k in keyed) void k; await pause(); return 1; });
});
