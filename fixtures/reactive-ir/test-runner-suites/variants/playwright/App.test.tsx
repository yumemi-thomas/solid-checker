import { createRenderEffect, createRoot, createSignal } from "solid-js";
import { test } from "@playwright/test";

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

// playwright calls the suite body inside `test.describe` (`body()`), so it
// runs under the root: its effect is owned, and the dispatch runs on an owned
// stack. The handler's effect stays uncertifiable.
createRoot(() => {
  test.describe("card", () => {
    createRenderEffect(() => count(), () => {});
    input.dispatchEvent(new Event("change"));
  });
});
