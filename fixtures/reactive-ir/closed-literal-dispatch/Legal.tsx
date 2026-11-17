import { createSignal } from "solid-js";

const [, setW] = createSignal(0);

// Clean: the literal's method hands its argument to a microtask, whatever the assertion says (ADR 0781's `cast-late-foreach`).
export function Late() {
  const t = { forEach(f: () => void) { queueMicrotask(f); } };
  (t as unknown as number[]).forEach(() => { setW(1); });
  return <p />;
}

export function LateThen() {
  const t = { then(f: () => void) { queueMicrotask(f); } };
  (t as unknown as Promise<void>).then(() => { setW(2); });
  return <p />;
}
