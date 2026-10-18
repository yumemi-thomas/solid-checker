import { createSignal } from "solid-js";

function rowProps(onKey: () => void) {
  return { onKeyDown: () => onKey() };
}

// 3a: outer object reached through a spread has a `this`-using getter that
// calls the event member while the element reads its attributes.
export function SpreadThis() {
  const [count] = createSignal(0);
  const p = {
    ...rowProps(() => console.log(count())),
    get title(): string {
      (this as { onKeyDown: () => void }).onKeyDown();
      return "t";
    },
  };
  return <div {...p}>a</div>;
}

// 3b: `Function.prototype.call` read off the callback and run in the body.
function keep(cb: () => void) {
  const run = cb.call.bind(cb);
  run(undefined);
  return { onClick: () => cb() };
}
export function CallRead() {
  const [count] = createSignal(0);
  const h = keep(() => console.log(count()));
  return <button {...h}>b</button>;
}

// 3c: [].forEach with cb.call as the callback, `cb` as thisArg.
function keep2(cb: () => void) {
  [0].forEach(cb.call, cb);
  return { onClick: () => cb() };
}
export function ForEachCall() {
  const [count] = createSignal(0);
  const h = keep2(() => console.log(count()));
  return <button {...h}>c</button>;
}

// 3d: named function expression escapes through its own name.
const slot: { fn?: () => void } = {};
export function Reader() {
  slot.fn?.();
  return <p>r</p>;
}
function wrap(cb: () => void) {
  return { onClick: cb };
}
export function SelfName() {
  const [count] = createSignal(0);
  const h = wrap(function me() {
    console.log(count());
    slot.fn = me;
  });
  return <button {...h}>d</button>;
}
