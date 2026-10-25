import { createRenderEffect, createRoot, createSignal } from "solid-js";

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

// jest-circus calls the suite body inside `describe` (blockFn()), so it runs
// under the root: its effect is owned, and the dispatch runs on an owned stack,
// where it may deliver any handler. The handler's effect stays uncertifiable.
createRoot(() => {
  describe("card", () => {
    createRenderEffect(() => count(), () => {});
    input.dispatchEvent(new Event("change"));
  });
});
