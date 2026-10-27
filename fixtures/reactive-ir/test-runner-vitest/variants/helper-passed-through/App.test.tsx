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

function choose(target: HTMLElement) {
  target.dispatchEvent(new Event("change"));
}

// The helper is handed on as a value to a project function that only tests
// call: the one call that runs it is written in that function, so every
// invocation is still a test's. The dispatch leaves the veto.
function forEachTarget(run: (target: HTMLElement) => void) {
  run(node);
}

describe("card", () => {
  it("chooses", () => {
    forEachTarget(choose);
  });
});
