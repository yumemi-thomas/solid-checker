import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1125 (round 1 F2 of ADR 1089 E, on main's read proof): a replaced `addEventListener` keeps the listener and the apply runs it.
const taken: EventListener[] = [];
const add = EventTarget.prototype.addEventListener;
EventTarget.prototype.addEventListener = function (this: EventTarget, type: string, listener: EventListenerOrEventListenerObject | null, options?: boolean | AddEventListenerOptions) {
  if (typeof listener === "function") taken.push(listener);
  return add.call(this, type, listener, options);
};

export function ListenerPatch() {
  createEffect(
    () => 1,
    () => {
      for (const run of taken) run(new Event("scroll"));
    },
  );
  return <div onScroll={() => { count(); }} />;
}
