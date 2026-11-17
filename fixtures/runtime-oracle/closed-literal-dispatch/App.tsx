import { createSignal } from "solid-js";

// ADR 1023: a member call on a closed `const` object literal runs the method written there, whatever type the receiver is asserted to have. Each
// literal's method below calls its argument in place, so the write or read in the callback runs in the component body: Chrome rc.21 reports each
// (REACTIVE_WRITE_IN_OWNED_SCOPE, STRICT_READ_UNTRACKED), and the checker proves each (uncertifiable before, `function-run-unplaced`).
const [w, setW] = createSignal(0);

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

const call = (g: () => void) => { g(); };
export function Wrapper() {
  const t = { then(f: () => void) { call(f); } };
  const run = () => { setW(4); };
  (t as unknown as Promise<void>).then(run);
  return <p />;
}

let kept: (() => void) | undefined;
export function AlsoKept() {
  const t = { then(f: () => void) { f(); kept = f; } };
  (t as unknown as Promise<void>).then(() => { setW(5); });
  return <p />;
}

// Legal: the method hands its argument to a microtask, so the write runs outside the body: Chrome is quiet and the checker is clean (it was
// uncertifiable before).
export function Late() {
  const t = { forEach(f: () => void) { queueMicrotask(f); } };
  (t as unknown as number[]).forEach(() => { setW(6); });
  return <p />;
}

export function LateThen() {
  const t = { then(f: () => void) { queueMicrotask(f); } };
  (t as unknown as Promise<void>).then(() => { setW(7); });
  return <p />;
}
