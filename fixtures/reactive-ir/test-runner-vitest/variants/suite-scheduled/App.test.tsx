import { createRenderEffect, createRoot, createSignal } from "solid-js";
import { describe } from "vitest";

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

// vitest records the suite and runs its body later, from `collect` after the
// file has loaded (`runWithSuite`), wherever the registration is written: the
// root does not own the effect (SC4001), and the dispatch leaves the veto.
createRoot(() => {
  describe("card", () => {
    createRenderEffect(() => count(), () => {});
    input.dispatchEvent(new Event("change"));
  });
});
