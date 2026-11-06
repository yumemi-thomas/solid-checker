// W's census review, H1-H5: each certified while Chrome warned. Each answers a `value-*` uncertifiable now.
import { For, createMemo, createSignal, createStore } from "solid-js";
import array from "./ReviewArray";
import object from "./ReviewObject";
import created from "./ReviewCreated";

// H1: `export default` of a literal or a creating call names nothing an importer's walk starts from.
export function DefaultArray() {
  try { throw array[0]; } catch (e) { (e as () => number)(); }
  return <p>x</p>;
}
export function DefaultObject() {
  try { throw object.c; } catch (e) { (e as () => number)(); }
  return <p>x</p>;
}
export function DefaultCreated() {
  try { throw created; } catch (e) { (e as () => number)(); }
  return <p>x</p>;
}

// H2: inside a parameter hop, a pattern over a holder has no row.
function run(f: () => number) {
  const o = { f };
  const { f: g } = o;
  return g();
}
export function ParamPattern() {
  const [c] = createSignal(1);
  run(c);
  return <p>x</p>;
}

// H3: a tracked position does not excuse a pending accessor.
export function PendingPattern() {
  const c = createMemo(async () => { await new Promise((r) => setTimeout(r, 20)); return 1; });
  const o = { c };
  const { c: h } = o;
  return <p>{h()}</p>;
}

// H4: a store root's member reached through a slot needs a row.
export function StoreSlot() {
  const [st] = createStore({ a: 1 });
  const o = { st };
  void o.st.a;
  return <p>x</p>;
}

// H5: a bare child is what its parent does with it: `For` calls it as the mapper.
export function ForChild() {
  const [c] = createSignal(1);
  return <For each={[1]}>{c}</For>;
}
