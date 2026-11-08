import { it } from "vitest";
import { flush } from "solid-js";

// The same helper, also called from a component body: nothing places the
// dispatch, so a flush in a handler stays an obligation.
function press(): void {
  window.dispatchEvent(new Event("keydown"));
}

export function Pressed() {
  press();
  return <button onClick={() => flush()}>x</button>;
}

it("presses", () => {
  press();
});
