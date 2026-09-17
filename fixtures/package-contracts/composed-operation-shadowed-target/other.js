import { createSignal } from "solid-js";

// A second declaration of the *same name*, in a second module. Nothing here
// may answer for `readSignal` in index.js, and provenance must name this one
// by identity rather than by the name the two share.
function readSignal() {
  const [elsewhere] = createSignal("elsewhere");
  return elsewhere();
}

export { readSignal as otherReadSignal };
