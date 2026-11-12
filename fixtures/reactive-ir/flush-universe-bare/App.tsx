import { flush } from "solid-js";

// Batch 11, slice 5: a program that names no primitive running a callback as an action's steps, deferring one to a later flush, or running one under a leaf
// owner (`project_steps_action_callbacks`, `project_defers_callbacks`, `project_opens_leaf_scope`) has no step, no apply and no leaf callback on any stack: each
// of the three codes ends on its own fact (`NoSteppedAction`, `NoDeferringPrimitive`, `NoLeafScope`), and the flush has no row.
function save() {
  flush();
}

export function Button() {
  return <button onClick={() => save()}>go</button>;
}
