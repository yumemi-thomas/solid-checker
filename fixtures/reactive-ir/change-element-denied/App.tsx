import { createEffect, createSignal, createStore } from "solid-js";

// ADR 1146: only a literal checkbox or radio takes the proof. Each handler below creates an effect, on a line of its own: where the proof does not hold it
// is uncertifiable (`denied-event`), as for a text control, which Chrome delivers `change` at `blur()`, `focus()` elsewhere, `remove()` and `disabled = true`,
// in the call.
export function Denied() {
  const [s] = createStore({ v: 0 });
  const [kind] = createSignal("checkbox");
  const props = { title: "t" };
  let ref: HTMLInputElement | undefined;
  return (
    <form onChange={() => { createEffect(() => s.v, () => {}); }}>
      <input type="text" onChange={() => { createEffect(() => s.v, () => {}); }} />
      <input onChange={() => { createEffect(() => s.v, () => {}); }} />
      <input type="search" onChange={() => { createEffect(() => s.v, () => {}); }} />
      <select onChange={() => { createEffect(() => s.v, () => {}); }} />
      <textarea onChange={() => { createEffect(() => s.v, () => {}); }} />
      <input type={kind()} onChange={() => { createEffect(() => s.v, () => {}); }} />
      <input type="checkbox" {...props} onChange={() => { createEffect(() => s.v, () => {}); }} />
      <input type="checkbox" ref={ref} onChange={() => { createEffect(() => s.v, () => {}); }} />
      <input type="checkbox" prop:type="text" onChange={() => { createEffect(() => s.v, () => {}); }} />
      <input type="checkbox" type="text" onChange={() => { createEffect(() => s.v, () => {}); }} />
      <input type="checkbox" is="x-box" onChange={() => { createEffect(() => s.v, () => {}); }} />
      <button type="button" onChange={() => { createEffect(() => s.v, () => {}); }} />
    </form>
  );
}
