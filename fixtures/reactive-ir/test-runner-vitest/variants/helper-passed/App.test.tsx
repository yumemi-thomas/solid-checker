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

const handlers: Array<(target: HTMLElement) => void> = [];

function choose(target: HTMLElement) {
  target.dispatchEvent(new Event("change"));
}

// The helper is stored as a value: whoever reads the array may call it from
// anywhere, a component body included. The veto stays.
handlers.push(choose);

describe("card", () => {
  it("chooses", () => {
    choose(node);
  });
});
