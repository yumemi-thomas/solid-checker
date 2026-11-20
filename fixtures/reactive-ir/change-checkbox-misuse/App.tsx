import { createEffect, createStore } from "solid-js";

// ADR 1146: the handler of a literal checkbox or radio runs with no owner, so a `createEffect` in it is a proven SC4001 `missing-owner` violation
// (Chrome: NO_OWNER_EFFECT, runtime-oracle `change-checkbox-misuse`). Before the proof it was uncertifiable (`denied-event`).
export function Misuse() {
  const [s] = createStore({ v: 0 });
  return (
    <form>
      <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />
      <input type="radio" name="r" onChange={() => { createEffect(() => s.v, () => {}); }} />
    </form>
  );
}
