import { createRenderEffect, createSignal } from "solid-js";
import { afterAll, beforeEach, describe, it, test } from "vitest";

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

// Every registration form of the premise, each with a dispatch.
describe.skip("suite", () => {
  beforeEach(() => {
    input.dispatchEvent(new Event("a"));
  });
  afterAll(async () => {
    input.dispatchEvent(new Event("b"));
  });
  it.only("only", () => {
    input.dispatchEvent(new Event("c"));
  });
  test.skip("skip", () => {
    input.dispatchEvent(new Event("d"));
  });
  it.each([1, 2])("each %i", (value) => {
    input.dispatchEvent(new Event(String(value)));
  });
});
