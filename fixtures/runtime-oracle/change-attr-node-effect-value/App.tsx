import { createEffect, createSignal, createStore } from "solid-js";

// ADR 1148 (round 1, F1, p03): a createEffect handler on a checkbox beside a function that writes the control's Attr node's value. The click path of the checkbox creates the
// effect with no owner: Chrome warns NO_OWNER_EFFECT, so the violation the checker proved before the Attr routes were vetoed was true on that run.
export function Probe() {
  const [s] = createStore({ v: 0 });
  const [n, setN] = createSignal(0);
  return (
    <form>
      <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />
      <input type="radio" name="r" onChange={() => setN(n() + 1)} />
    </form>
  );
}

export function retype() {
  const a = document.evaluate("//input/@type", document, null, 9, null).singleNodeValue as Attr;
  a.value = "text";
}
