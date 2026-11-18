// ADR 1089 part E round 2: the Fable review's case (coord-land/review-1089e.md, r1089e-setter). Chrome rc.21 reports
// PRIMITIVE_IN_EFFECT_CALLBACK at the creation in the handler: the handler-layer proof must not hold here.
import { createEffect, resolve } from "solid-js";

// A setter on the prototype takes the handler Solid stores at `node["_$$" + name]`. The key is built as Solid builds it.
const taken: Array<() => void> = [];
Object.defineProperty(Element.prototype, "_$$" + "click", {
  configurable: true,
  set(this: Element, handler: () => void) {
    taken.push(handler);
  },
});

export function SetterCapture() {
  createEffect(
    () => 1,
    () => {
      for (const run of taken) run();
    },
  );
  return (
    <button
      onClick={() => {
        resolve(() => 1);
      }}
    >
      go
    </button>
  );
}
