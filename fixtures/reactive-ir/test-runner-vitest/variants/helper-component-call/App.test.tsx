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

// A helper the tests call and a component body calls too: in the component's
// body it runs under the component's owner, so its dispatch may deliver the
// handler under that owner. The veto stays.
function choose(target: HTMLElement) {
  target.dispatchEvent(new Event("change"));
  setCount(count() + 1);
}

export function Panel() {
  choose(node);
  return <div />;
}

describe("card", () => {
  it("chooses", () => {
    choose(node);
  });
});
