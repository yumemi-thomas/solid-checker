import { createSignal } from "solid-js";

const [, setW] = createSignal(0);

// The module's `t` hands its argument to a microtask; a module-level receiver is not followed, so its call stays uncertifiable.
const t = { then(f: () => void) { queueMicrotask(f); } };
export function Outer() {
  (t as unknown as Promise<void>).then(() => { setW(1); });
  return <p />;
}

// Proved: the component's own `t` shadows it and runs its argument in place.
export function Shadowed() {
  const t = { then(f: () => void) { f(); } };
  (t as unknown as Promise<void>).then(() => { setW(2); });
  return <p />;
}

// Not followed yet: a `t` shadowed in a nested block (uncertifiable at both calls).
export function BlockShadowed() {
  const t = { then(f: () => void) { queueMicrotask(f); } };
  {
    const t = { then(f: () => void) { f(); } };
    (t as unknown as Promise<void>).then(() => { setW(3); });
  }
  (t as unknown as Promise<void>).then(() => { setW(4); });
  return <p />;
}
