import { createRenderEffect, createSignal } from "solid-js";
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

// The body runs inside `test.describe`, here at module evaluation: no owner is
// current, so the effect has none (SC4001), and the dispatch is owner-free.
test.describe("card", () => {
  createRenderEffect(() => count(), () => {});
  input.dispatchEvent(new Event("change"));
});
