// ADR 1089 part E round 2: the Fable review's case (coord-land/review-1089e.md, r1089e-listener). Chrome rc.21 reports
// PRIMITIVE_IN_EFFECT_CALLBACK at the creation in the handler: the handler-layer proof must not hold here.
import { createEffect, resolve } from "solid-js";

// A patched addEventListener keeps the listener Solid adds for an event it does not delegate.
const taken: EventListener[] = [];
const add = EventTarget.prototype.addEventListener;
EventTarget.prototype.addEventListener = function (
  this: EventTarget,
  type: string,
  listener: EventListenerOrEventListenerObject | null,
  options?: boolean | AddEventListenerOptions,
) {
  if (type === "scroll" && typeof listener === "function") taken.push(listener);
  return add.call(this, type, listener, options);
};

export function PatchedAddEventListener() {
  createEffect(
    () => 1,
    () => {
      for (const run of taken) run(new Event("scroll"));
    },
  );
  return (
    <div
      onScroll={() => {
        resolve(() => 1);
      }}
    />
  );
}
