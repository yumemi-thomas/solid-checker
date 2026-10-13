import { createEffect, createRenderEffect, onCleanup } from "solid-js";

// `createEventListener`'s shape (`@solid-primitives/event-listener@3.0.0-next.5`):
// one of two effect constructors runs on every call, so every call registers
// a computation although neither call is unconditional.
export function listen(target) {
  const compute = () => target;
  const apply = () => {};
  if (typeof target === "function") createEffect(compute, apply);
  else createRenderEffect(compute, apply);
}

// No `else`: a call with a falsy flag registers nothing.
export function maybeListen(flag) {
  const apply = () => {};
  if (flag) createEffect(() => flag, apply);
}

// Two roles, one per arm: every call registers something, but neither a
// computation nor a cleanup on every call.
export function effectOrCleanup(flag) {
  const apply = () => {};
  if (flag) createEffect(() => flag, apply);
  else onCleanup(apply);
}

// An early return before both arms: the cover holds only past the guard.
export function guardedListen(target, skip) {
  if (skip) return;
  const apply = () => {};
  if (typeof target === "function") createEffect(() => target, apply);
  else createRenderEffect(() => target, apply);
}
