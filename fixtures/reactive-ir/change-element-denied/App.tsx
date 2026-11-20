import { createEffect, createSignal, createStore } from "solid-js";

// ADR 1148: only a literal checkbox or radio takes the proof. Each handler below creates an effect, on a line of its own: where the proof does not hold it
// is uncertifiable (`denied-event`), as for a text control, which Chrome delivers `change` at `blur()`, `focus()` elsewhere, `remove()` and `disabled = true`,
// in the call. The runtime performs the writes a non-constant `type` (`type={kind()}`), a spread (`{...props}`, which carries a `type`) and `prop:type` stand for, on the
// element, and the proof never reads the runtime's source: so the element rule, not the program rule, withdraws it, one element for each. (`attr:type` is
// `change-attr-namespace`: Solid 2 removed the namespace and the compiler refuses the file.)
export function Denied() {
  const [s] = createStore({ v: 0 });
  const [kind] = createSignal("checkbox");
  const props = { type: "text" };
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
      <input {...props} type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />
      <input type={"checkbox"} onChange={() => { createEffect(() => s.v, () => {}); }} />
      <button type="button" onChange={() => { createEffect(() => s.v, () => {}); }} />
    </form>
  );
}
