import { createMemo, createSignal } from "solid-js";
import { h1 } from "./helpers";

// ADR 1186 (census mx-reactive-write-in-owned-scope-memo-shared-signal-loop-callee-module-helper): the setter is held in `fns` and
// run by `loop`'s call through `f`, in the memo's compute (Chrome REACTIVE_WRITE_IN_OWNED_SCOPE at that call) and in a handler.
// SC9012 keeps its row at `fns` and now has one at the call `(f as any)(...xs)` too, where Chrome's frame is.

export function C1() {
  const [w1, setW1] = createSignal(0);
  const fns = [setW1];
  const loop = (...xs: any[]) => { for (const f of fns) return (f as any)(...xs); };
  const g1 = () => {
    h1(() => {
      loop(7);
    });
  };
  const m1 = createMemo(() => {
    g1();
    return 1;
  });
  return (
    <div>
      <p data-view="v1">{m1()}</p>
      <button data-act="b1" onClick={() => {
        g1();
      }}>go</button>
    </div>
  );
}

// No carrier: a direct setter call in a handler is clean.
export function DirectInHandler() {
  const [, setW] = createSignal(0);
  return <button onClick={() => setW(1)}>go</button>;
}
