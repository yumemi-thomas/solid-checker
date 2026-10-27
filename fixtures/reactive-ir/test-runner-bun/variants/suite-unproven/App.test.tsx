import { createRenderEffect, createSignal } from "solid-js";
import { describe } from "bun:test";

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

// bun's runner is native: no published JavaScript shows whether the suite body
// runs inside `describe` or after the file loads (bun 1.4.0 defers it, which a
// probe showed; an earlier release may not), so the body is not placed. Its
// effect and the handler's stay uncertifiable, and the dispatch keeps the veto.
describe("card", () => {
  createRenderEffect(() => count(), () => {});
  input.dispatchEvent(new Event("change"));
});
