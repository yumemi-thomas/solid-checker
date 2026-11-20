import { createEffect, createStore } from "solid-js";

// ADR 1146: the handler of a literal checkbox or radio runs with no owner (the oracle's synthetic click activates the control and delivers `change` to it),
// so a `createEffect` in it warns NO_OWNER_EFFECT: the checker's violation at each site is confirmed.
export function Misuse() {
  const [s] = createStore({ v: 0 });
  return (
    <form>
      <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />
      <input type="radio" name="r" onChange={() => { createEffect(() => s.v, () => {}); }} />
    </form>
  );
}
