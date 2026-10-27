import { createRenderEffect, createSignal } from "solid-js";
import { describe } from "node:test";

const [count] = createSignal(0);
const input = document.createElement("input");

export function Card() {
  return (
    <button
      onClick={() => {
        createRenderEffect(() => count(), () => {});
      }}
    />
  );
}

// node:test runs the suite body inside the `describe` call, here at module
// evaluation: no owner is current, so the effect has none (SC4001), and the
// dispatch is owner-free: the handler's effect, delivered by the user, is the
// second violation.
describe("card", () => {
  createRenderEffect(() => count(), () => {});
  input.dispatchEvent(new Event("change"));
});
