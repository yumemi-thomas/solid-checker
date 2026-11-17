import { createSignal } from "solid-js";

const [w, setW] = createSignal(0);

// Proved: the literal's own `then` calls its argument on every completion, in the component body.
export function Cast() {
  const t = { then(f: () => void) { f(); } };
  (t as unknown as Promise<void>).then(() => { setW(1); });
  return <p />;
}

export function CastAny() {
  const t = { then(f: () => void) { f(); } } as any;
  (t as Promise<void>).then(() => { setW(2); });
  return <p />;
}

export function ArrowMember() {
  const t = { then: (f: () => void) => { f(); } };
  (t as unknown as Promise<void>).then(() => { setW(3); });
  return <p />;
}

export function Read() {
  const t = { forEach(f: () => void) { f(); } };
  (t as unknown as number[]).forEach(() => { w(); });
  return <p />;
}

// Wrapper: the callback is a named function, and the method forwards its argument to a helper that calls it.
const call = (g: () => void) => { g(); };
export function Wrapper() {
  const t = { then(f: () => void) { call(f); } };
  const run = () => { setW(4); };
  (t as unknown as Promise<void>).then(run);
  return <p />;
}

// Member: the method also keeps its argument; the run in place is still one run.
let kept: (() => void) | undefined;
export function AlsoKept() {
  const t = { then(f: () => void) { f(); kept = f; } };
  (t as unknown as Promise<void>).then(() => { setW(5); });
  return <p />;
}
