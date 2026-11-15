import { createMemo, createStore, Loading } from "solid-js";

const [store] = createStore({ n: { x: 1 } });
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// ADR 1000: a write that reads its target first is a read of it. After an `await`, a compound assignment, an update and a logical assignment on a
// store alias each Get the member before they Set it: Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT at each (and the store write, which is SC2003's).
// The member was a write target, so the after-await member list skipped it. Each read is written on one line.

// U (read-after-await-unaccounted): `+=` reads `local.x` first.
export function Compound() {
  const m = createMemo(async () => { const local = store.n; await pause(); local.x += 1; return 1; });
  return <Loading fallback="w"><p>{String(m())}</p></Loading>;
}

// U: `++` reads it first.
export function Update() {
  const m = createMemo(async () => { const local = store.n; await pause(); local.x++; return 1; });
  return <Loading fallback="w"><p>{String(m())}</p></Loading>;
}

// U: `||=` reads it first.
export function Logical() {
  const m = createMemo(async () => { const local = store.n; await pause(); local.x ||= 5; return 1; });
  return <Loading fallback="w"><p>{String(m())}</p></Loading>;
}

// no SC1002 row: a plain assignment reads nothing (the write is SC2003's).
export function Plain() {
  const m = createMemo(async () => { const local = store.n; await pause(); local.x = 2; return 1; });
  return <Loading fallback="w"><p>{String(m())}</p></Loading>;
}

// no SC1002 row: the same compound assignment on a plain object the function made.
export function Local() {
  const m = createMemo(async () => { const local = { x: 1 }; await pause(); local.x += 1; return local.x; });
  return <Loading fallback="w"><p>{String(m())}</p></Loading>;
}
