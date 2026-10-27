import { createRenderEffect, createRoot, createSignal } from "solid-js";
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

// node:test builds a suite inside the `describe` call (the Suite constructor
// runs the body), so this body runs under the root: its effect is owned, and
// the dispatch runs on an owned stack, where it may deliver any handler. The
// handler's effect stays uncertifiable.
createRoot(() => {
  describe("card", () => {
    createRenderEffect(() => count(), () => {});
    input.dispatchEvent(new Event("change"));
  });
});
