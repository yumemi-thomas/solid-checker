import { createMemo, createSignal, flush, onSettled } from "solid-js";

const [count] = createSignal(0);

export function NestedMemo() {
  // A memo created in an onSettled nested in another onSettled runs out of band: no throw.
  onSettled(() => { onSettled(() => { createMemo(() => count()); }); });
  return <i />;
}

export function NestedFlush() {
  // flush() in the same out-of-band position is fine too.
  onSettled(() => { onSettled(() => { flush(); }); });
  return <i />;
}

export function OwnedMemo() {
  // Control: a memo in an owned onSettled body throws PRIMITIVE_IN_FORBIDDEN_SCOPE (SC3001).
  onSettled(() => { createMemo(() => count()); });
  return <i />;
}

function settledMemo() {
  onSettled(() => { createMemo(() => count()); });
}

export function HelperBothWays() {
  // settledMemo runs owned here and from an owned leaf there: the owner is conditional, so the
  // helper's onSettled gate is uncertain, not a violation.
  settledMemo();
  onSettled(() => { settledMemo(); });
  return <i />;
}
