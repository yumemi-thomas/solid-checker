import { createRenderEffect, createSignal } from "solid-js";
import { describe, it } from "vitest";

const [count, setCount] = createSignal(0);
const node = document.createElement("input");

// The subject: a handler on an intrinsic element. Its write and read are legal
// event work when no dispatch can deliver it under an owner.
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

describe("card", () => {
  // The dispatch follows an await: the continuation runs with no owner.
  it("delivers after an await", async () => {
    const target = await open();
    target.dispatchEvent(new Event("change"));
  });

  // The prefix before the first await and a loop that awaits at every
  // resumption: no owner anywhere in the body.
  it("delivers in a loop", async () => {
    node.dispatchEvent(new Event("input"));
    for await (const item of [open()]) {
      item.dispatchEvent(new Event("change"));
    }
  });

  // A read and a write in the continuation are scheduled-callback work.
  it("reads and writes after an await", async () => {
    await open();
    setCount(count() + 1);
  });
});
