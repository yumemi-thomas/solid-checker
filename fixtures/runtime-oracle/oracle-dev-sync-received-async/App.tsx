// SC7002 sync-computation-received-async. The dev build's handleAsync emits
// SYNC_NODE_RECEIVED_ASYNC and throws when a sync: true node's compute returns a
// promise. The violation creates the memo from a timer, so its first compute
// runs inside the createMemo call the diagnostic is attributed to. The control
// is a synchronous sync: true memo.
import { createMemo } from "solid-js";

export function Violation() {
  setTimeout(() => {
    createMemo(async () => 1, { sync: true });
  }, 0);
  return <div />;
}

export function Control() {
  const value = createMemo(() => 1, { sync: true });
  return <div>{value()}</div>;
}
