import { createRenderEffect, createSignal } from "solid-js";
import { it } from "vitest";

const [count] = createSignal(0);

// `skipIf(condition)` takes data and returns the registration; only the
// outermost call, `…("x", () => {})`, registers a callback. The condition
// function is handed to a call that is not a registration, so it is not
// placed on the runner's scheduler, and its effect is owned by the component.
export function Rows() {
  it.skipIf(() => {
    createRenderEffect(() => count(), () => {});
    return false;
  })("x", () => {});
  return <div />;
}

// The registration's own callback is the runner's: no owner (SC4001).
export function Registers() {
  it.skipIf(false)("y", () => {
    createRenderEffect(() => count(), () => {});
  });
  return <div />;
}
