import { createSignal, untrack } from "solid-js";

const [b, setB] = createSignal(0);

// Positive: `ownedWrite: true` skips the owned-scope guard (rc.21
// `setSignal`), so the updater runs at the call, in the body's window.
export function Owned() {
  const [a, setA] = createSignal(0, { ownedWrite: true });
  setA(() => b());
  return <span>{a()}</span>;
}

// Uncertifiable: an options value the syntax does not show may set it.
const options = { ownedWrite: true };
export function OptionsUnread() {
  const [a, setA] = createSignal(0, options);
  setA(() => b());
  return <span>{a()}</span>;
}

// Negative (the read): a plain setter's guard throws first, so the updater
// never runs; the write itself is SC2001's.
export function Plain() {
  const [a, setA] = createSignal(0);
  setA(() => b());
  return <span>{a()}</span>;
}

// Negative (the read): `ownedWrite: false` is the plain setter.
export function OwnedOff() {
  const [a, setA] = createSignal(0, { ownedWrite: false });
  setA(() => b());
  return <span>{a()}</span>;
}

// Negative: the updater's read is an explicit snapshot.
export function OwnedUntracked() {
  const [a, setA] = createSignal(0, { ownedWrite: true });
  setA(() => untrack(b));
  return <span>{a()}</span>;
}

// Negative: in a handler, no window is open.
export function OwnedInHandler() {
  const [a, setA] = createSignal(0, { ownedWrite: true });
  return <button onClick={() => setA(() => b())}>{a()}</button>;
}

// A module-scope write: no owner, no window.
setB(() => b() + 1);
