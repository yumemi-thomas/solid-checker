import { createEffect, createSignal, createTrackedEffect, flush } from "solid-js";

const [n] = createSignal(0);

// Batch 11, slice 5: a program that names no primitive running a callback as an action's steps (`project_steps_action_callbacks`) has no step on any stack,
// so SC2006 ends every flush on that fact (`NoSteppedAction`). It runs an effect's apply and opens a leaf scope, so SC2008 and SC3001 stand on the census
// class of the flush's own caller, and the helper below (every invocation an event delivery) has none that rules them out.
export function Effect() {
  createEffect(() => n(), () => {});
  return <div />;
}

export function Leaf() {
  createTrackedEffect(() => {
    n();
  });
  return <div />;
}

function save() {
  flush();
}

export function Button() {
  return <button onClick={() => save()}>go</button>;
}
