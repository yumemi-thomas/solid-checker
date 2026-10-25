import { createSignal } from "solid-js";
import { it } from "vitest";

const [count] = createSignal(0);

// A project function that calls its callback at once, inside the component
// body's strict-read window.
function runLater(name: string, body: () => void) {
  body();
}

// The runner invokes its callback from its own scheduler, outside any
// strict-read window: the read is clean.
export function Registers() {
  it("reads", () => {
    count();
  });
  return <div />;
}

// The same shape with a project function that calls its callback inline: the
// read runs in the component body's strict-read window (SC1001).
export function Unplaced() {
  runLater("reads", () => {
    count();
  });
  return <div />;
}
