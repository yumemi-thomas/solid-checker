import { isReady, isReadyOpen } from "reactive-package";

// The claim under test. `isReady`'s accepted contract closes `returns` over one
// `return` whose output is `plain` (ADR 0113): what the call hands back carries
// no reactive capability, so reading it here asks nothing of the package, and
// the import finds no claim open.
export function Status() {
  const ready = isReady();
  return <div>{ready ? "ready" : "waiting"}</div>;
}

// The control. The same declaration and the same use, with `returns` left open:
// the value could be anything the package hands back, so the import reports the
// open claim. The closure above has to remove exactly this finding, and nothing
// else.
export function StatusOpen() {
  const ready = isReadyOpen();
  return <div>{ready ? "ready" : "waiting"}</div>;
}
