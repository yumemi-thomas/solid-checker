import { createRenderEffect, createSignal } from "solid-js";
import { describe, it } from "vitest";

const [count, setCount] = createSignal(0);
const input = document.createElement("input");

// The subject: a handler on an intrinsic element. Whether its write and its
// read are proven event-only, and its effect proven ownerless, is what the
// dispatch in the test below decides.
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

describe("card", () => {
  it("delivers a change", () => {
    input.dispatchEvent(new Event("change"));
  });
});
