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

// Review R13: past the walk's bound the entries are unread: open, whatever the
// role at the call. Each function is called twice, so no in-place climb ends it.
export function PastTheBound() {
  const h1 = () => { resolve(() => 1); };
  const h2 = () => { h1(); h1(); };
  const h3 = () => { h2(); h2(); };
  const h4 = () => { h3(); h3(); };
  const h5 = () => { h4(); h4(); };
  return <p>{(h5(), 1)}</p>;
}

// Review R16: an `async` function's prefix runs on the call's stack.
export function AsyncPrefix() {
  const h = async () => { resolve(() => 1); };
  return <p>{(void h(), 1)}</p>;
}

// Past the await no observer is active.
export function AsyncSuffix() {
  const h = async () => { await 0; resolve(() => 1); };
  return <p>{(void h(), 1)}</p>;
}

// Calling a generator runs none of it: the step is what runs it.
export function Stepped() {
  function* g() { resolve(() => 1); yield 1; }
  return <p>{(g().next(), 1)}</p>;
}
