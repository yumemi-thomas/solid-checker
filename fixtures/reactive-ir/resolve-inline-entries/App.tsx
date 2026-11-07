import { createMemo, flush, resolve, until, untrack } from "solid-js";

// `flush(fn)` runs `fn` before it drains, under the caller's observer.
export function FlushInJsx() {
  return <p>{(() => { flush(() => { resolve(() => 1); }); return 1; })()}</p>;
}

export function FlushInMemo() {
  const m = createMemo(() => { flush(() => { until(() => true); }); return 1; });
  return <p>{m()}</p>;
}

// A function the expression runs: a call; `[1].map` runs it once at least, but
// `Row` returns JSX, so the census takes it for a component: open.
export function Called() {
  const h = () => { resolve(() => 1); return 1; };
  return <p>{h()}</p>;
}

export function Mapped() {
  const Row = () => { flush(() => { resolve(() => 1); }); return <i />; };
  return <p>{(() => { [1].map(Row); return 1; })()}</p>;
}

// An empty receiver may run nothing: open.
export function Empty() {
  const Row = () => { flush(() => { resolve(() => 1); }); return <i />; };
  return <p>{(() => { ([] as number[]).map(Row); return 1; })()}</p>;
}

// Rendered as a component: the body has no observer. `untrack` clears it.
export function Tag() {
  const Row = () => { flush(() => { resolve(() => 1); }); return <i />; };
  return <p><Row /></p>;
}

export function Untracked() {
  return <p>{(() => { untrack(() => flush(() => { resolve(() => 1); })); return 1; })()}</p>;
}
