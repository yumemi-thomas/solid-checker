import { createRenderEffect, createSignal } from "solid-js";

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

// The body runs inside `describe`, here at module evaluation: no owner is
// current, so the effect has none (SC4001), and the dispatch is owner-free, so
// the handler's effect, delivered by the user, is the second violation.
describe("card", () => {
  createRenderEffect(() => count(), () => {});
  input.dispatchEvent(new Event("change"));
});
