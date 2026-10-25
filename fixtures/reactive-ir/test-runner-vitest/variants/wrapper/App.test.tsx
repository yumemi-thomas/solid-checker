import { createRenderEffect, createSignal } from "solid-js";
import { it } from "vitest";

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

// The runner is handed whatever `wrap` returns, which may run the arrow on its
// own stack: only a function literal written directly as the argument is the
// runner's callback.
function wrap(body: () => void) {
  return body;
}

it("delivers a change", wrap(() => {
  input.dispatchEvent(new Event("change"));
}));
