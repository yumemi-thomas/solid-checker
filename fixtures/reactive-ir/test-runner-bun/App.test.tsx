import { createRenderEffect, createSignal } from "solid-js";
import { describe, test } from "bun:test";

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

describe("card", () => {
  test("delivers a change", () => {
    input.dispatchEvent(new Event("change"));
  });
});
