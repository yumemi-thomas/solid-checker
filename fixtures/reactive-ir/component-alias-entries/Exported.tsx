import { createSignal } from "solid-js";

const [count] = createSignal(0);

// Uncertifiable: exported from an open program, so importers this build
// cannot see may render it; the visible call is not every entry.
export function Shared() {
  const value = count();
  return <b>{value}</b>;
}

// Positive: rendered in `Importer.tsx` through a renamed import and a local
// alias of it.
export function Imported() {
  const value = count();
  return <b>{value}</b>;
}

export function Host() {
  return <p>{Shared()}{Imported()}</p>;
}
