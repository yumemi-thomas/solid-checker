import { createRenderEffect, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);
const input = document.createElement("input");

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

// A project function named `it` is not a runner: nothing places its callback.
function it(name: string, body: () => void) {
  body();
}

it("delivers a change", () => {
  input.dispatchEvent(new Event("change"));
});
