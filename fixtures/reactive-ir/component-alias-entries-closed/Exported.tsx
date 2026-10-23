import { createSignal } from "solid-js";

const [count] = createSignal(0);

// Clean: in a closed program the module surface has no unseen importer, and
// the tracked call is every entry.
export function Shared() {
  const value = count();
  return <b>{value}</b>;
}

// Uncertifiable: `Importer.tsx` hands the import to `map`, an entry the
// census does not follow.
export function Lent() {
  const value = count();
  return <b>{value}</b>;
}

export function Host() {
  return <p>{Shared()}{Lent()}</p>;
}
