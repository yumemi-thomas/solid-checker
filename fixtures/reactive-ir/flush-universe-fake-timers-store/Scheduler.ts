import { flush } from "solid-js";

// Each callback is one a reviewed scheduler runs from an empty stack, written directly in it (the census's `FreshStack`). With no fake timers in the program the flush has
// no action step, effect apply or leaf callback below it, and all three codes end on `FlushSafe::FreshStack` (`flush-universe`'s `setTimeout`). A program that installs fake
// timers (`clock.ts`) runs the callbacks on the stack of whatever advances the clock, which may be an action's step: F1 of the review of #213 withholds the reason, so each flush
// keeps the uncertifiable row of each code (nine rows).
export function schedule() {
  setTimeout(() => {
    flush();
  }, 10);
  queueMicrotask(() => {
    flush();
  });
  Promise.resolve().then(() => {
    flush();
  });
}
