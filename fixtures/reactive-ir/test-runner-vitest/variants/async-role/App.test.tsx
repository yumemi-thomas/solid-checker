import { createSignal } from "solid-js";
import { it } from "vitest";

const [count, setCount] = createSignal(0);

// A project function that calls its callback at once, inside the component
// body's windows.
function runLater(name: string, body: () => Promise<void>) {
  void body();
}

async function settle() {}

// The runner invokes an async callback from its own scheduler: the prefix runs
// on the runner's stack and every continuation has no owner, so a read and a
// write, before the await, after it and in a loop that awaits, are clean.
export function Registers() {
  it("reads and writes", async () => {
    count();
    setCount(1);
    await settle();
    count();
    setCount(2);
    for await (const _ of [settle()]) {
      count();
      setCount(3);
    }
  });
  return <div />;
}

// The same body handed to a project function that calls it inline runs its
// prefix in the component body's windows: the read is SC1001 and the write
// SC2001. The continuation after the await is not placed.
export function Unplaced() {
  runLater("reads", async () => {
    count();
    setCount(1);
    await settle();
  });
  return <div />;
}
