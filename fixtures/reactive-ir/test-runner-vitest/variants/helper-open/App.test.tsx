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

// The program boundary is not declared closed (no .solid-checker/runtime.json),
// so a module this build does not see may call the helper. The veto stays.
function choose(target: HTMLElement) {
  target.dispatchEvent(new Event("change"));
}

describe("card", () => {
  it("chooses", () => {
    choose(node);
  });
});
