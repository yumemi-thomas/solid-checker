import { createRenderEffect, createSignal } from "solid-js";
import { it } from "vitest";

const [count] = createSignal(0);

// A project function that calls its callback at once, under the caller's
// owner.
function runLater(name: string, body: () => void) {
  body();
}

// The runner invokes its callback from its own scheduler, whatever the body
// that registers it: the effect has no owner (SC4001).
export function Registers() {
  it("creates an effect", () => {
    createRenderEffect(() => count(), () => {});
  });
  return <div />;
}

// The same shape with a project function that calls its callback inline: the
// effect is owned by the component, so nothing is reported.
export function Unplaced() {
  runLater("creates an effect", () => {
    createRenderEffect(() => count(), () => {});
  });
  return <div />;
}
