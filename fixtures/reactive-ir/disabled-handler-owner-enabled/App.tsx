import { createEffect, createStore } from "solid-js";

// ADR 1134 negative: the button is disabled from mount, but the program writes `.disabled` (here through a ref), so it may be
// enabled and its handler may run: the SC4001 missing-owner row stays a violation.
export function EnabledLater() {
  const [s1] = createStore({ v: 0 });
  let other!: HTMLButtonElement;
  setTimeout(() => { other.disabled = false; }, 0);
  return (
    <div>
      <button ref={other}>other</button>
      <button disabled onClick={() => { createEffect(() => s1.v, () => {}); }}>go</button>
    </div>
  );
}
