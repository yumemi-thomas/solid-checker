import { createSignal } from "solid-js";
import { Dynamic } from "@solidjs/web";

const [count] = createSignal(0);

// Unchanged (ADR 0331's census for a function no tag renders): stored in a
// map only `Dynamic` reads, with no tag of its own.
function Untagged() {
  const value = count();
  return <b>{value}</b>;
}
const untagged = { untagged: Untagged };

export function NoTag() {
  return <Dynamic component={untagged.untagged} />;
}
