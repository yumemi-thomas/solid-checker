import { createSignal } from "solid-js";

// Quoted key: `o.run` reads the same property as `"run"`.
function useQuoted(cb: () => void) {
  const o = { "run": cb };
  const r = o.run;
  r();
  return { onClick: () => cb() };
}
export function Quoted() {
  const [count] = createSignal(0);
  const h = useQuoted(() => console.log(count()));
  return <button {...h}>q</button>;
}

// Escaped identifier key.
function useEscaped(cb: () => void) {
  const o = { run: cb };
  const r = o.run;
  r();
  return { onClick: () => cb() };
}
export function Escaped() {
  const [count] = createSignal(0);
  const h = useEscaped(() => console.log(count()));
  return <button {...h}>e</button>;
}

// Quoted key on the returned holder, read back by its identifier name.
function useQuotedReturn(cb: () => void) {
  return { "onClick": cb, "label": "x" };
}
export function QuotedReturn() {
  const [count] = createSignal(0);
  const h = useQuotedReturn(() => console.log(count()));
  const f = h.onClick;
  f();
  return <button onClick={() => h.onClick()}>r</button>;
}

// Quoted later key overriding a spread: `{...a, "onClick": other}`.
function base(cb: () => void) {
  return { onClick: cb };
}
export function QuotedOverride() {
  const [count] = createSignal(0);
  const h = { ...base(() => console.log(count())), "onClick": () => undefined };
  return <button {...h}>o</button>;
}
