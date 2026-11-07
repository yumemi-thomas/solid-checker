import { createMemo, createSignal } from "solid-js";

function Foo() {}
Foo.prototype.other = function () { return 1; };

export const isPlain = (v: object) => {
  const gp = Object.getPrototypeOf(v);
  return gp === null || gp === Object.prototype;
};

export function App() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => {
    [1].forEach(() => { set(1); });
    Promise.resolve().then(() => { set(2); });
    return s();
  });
  return <p>{m()}</p>;
}
