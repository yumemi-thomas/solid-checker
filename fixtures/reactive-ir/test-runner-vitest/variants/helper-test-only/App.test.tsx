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

// A module-level helper whose only callers are test callbacks.
// It also reads and writes a signal: the runner calls it outside every window
// and owner, so both are clean.
function choose(target: HTMLElement) {
  target.dispatchEvent(new Event("change"));
  setCount(count() + 1);
}

// An async helper: the dispatch after its await runs with no owner whoever
// called it, and its only caller is a test.
async function settle(target: HTMLElement) {
  await open();
  target.dispatchEvent(new Event("blur"));
}

describe("card", () => {
  // A helper declared in the suite body, called only from tests of the suite
  // (and from the other helper's callers, transitively).
  function type(target: HTMLElement) {
    target.dispatchEvent(new Event("input"));
    again(target);
  }
  function again(target: HTMLElement) {
    target.dispatchEvent(new Event("input"));
  }

  it("types", () => {
    type(node);
  });

  it("chooses after an await", async () => {
    const target = await open();
    choose(target);
    await settle(target);
  });
});
