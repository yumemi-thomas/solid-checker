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

// A test, unlike a suite body, is only recorded by `describe`/`test` and run
// later from the runner's own loop (`add_test`, then `run`), wherever its
// registration is written: no owner (SC4001), and the dispatch leaves the veto.
createRoot(() => {
  test("delivers a change", () => {
    createRenderEffect(() => count(), () => {});
    input.dispatchEvent(new Event("change"));
  });
});
