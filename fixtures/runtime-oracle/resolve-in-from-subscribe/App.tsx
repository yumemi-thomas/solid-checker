// ADR 0997: `from`'s subscribe function runs where its iterator is started, inside the memo that consumes it: the observer guard of `resolve` and `until` throws there.
import { createMemo, createSignal, from, resolve, until } from "solid-js";

const [n] = createSignal(1);

// U (no observer proven either way): `from`'s subscribe function runs where the iterator is started.
export function InFromR() {
  const m = createMemo(() => from<number>((set) => { void resolve(() => n()); set(1); return () => {}; }));
  return <p>{String(m())}</p>;
}
export function InFromU() {
  const m = createMemo(() => from<number>((set) => { void until(() => n() > 0); set(1); return () => {}; }));
  return <p>{String(m())}</p>;
}
