// ADR 0500 review negative: a function value called through the hook holder replaces its useIt slot through `this`, so the write lands with no warning; the checker must not prove a violation.
import { createStore } from "solid-js";

export function C1() {
  const [s1] = createStore({ v: 0, a: { v: 0 } });
  const useThing = () => ({
    useIt: s1,
    reset: function (this: { useIt: { a: { v: number } } }) {
      this.useIt = { a: { v: 0 } };
    },
  });
  const thing = useThing();
  return (
    <div>
      <button data-act="b1" onClick={() => {
        thing.reset();
        thing.useIt.a.v = 7;
      }}>go</button>
    </div>
  );
}
