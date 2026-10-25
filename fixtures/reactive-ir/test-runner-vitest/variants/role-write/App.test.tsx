import { createSignal } from "solid-js";
import { it } from "vitest";

const [, setCount] = createSignal(0);

// A project function that calls its callback at once, under the caller's
// owner.
function runLater(name: string, body: () => void) {
  body();
}

// The runner invokes its callback from its own scheduler, whatever the body
// that registers it: the write has no owner, so it is legal.
export function Registers() {
  it("writes", () => {
    setCount(1);
  });
  return <div />;
}

// The same shape with a project function that calls its callback inline: the
// write runs under the component's owner (SC2001).
export function Unplaced() {
  runLater("writes", () => {
    setCount(1);
  });
  return <div />;
}
