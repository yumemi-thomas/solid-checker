import { createRenderEffect, createSignal } from "solid-js";
import { describe, it } from "vitest";

const [count, setCount] = createSignal(0);
const node = document.createElement("input");

// The subject: a handler on an intrinsic element. Its effect creation is a
// violation exactly when no dispatch can deliver it under an owner.
export function Card() {
  return (
    <button
      onClick={() => {
        setCount(count() + 1);
        createRenderEffect(() => count(), () => {});
      }}
    />
  );
}

async function open() {
  return node;
}

// vitest 4 declares `describe` and `it` in `@vitest/runner` and re-exports
// them from `vitest`: the registration resolves through that alias.
describe("card", () => {
  it("delivers after an await", async () => {
    const target = await open();
    target.dispatchEvent(new Event("change"));
  });
});
