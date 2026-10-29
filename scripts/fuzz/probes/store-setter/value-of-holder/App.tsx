// ADR 0500 review negative: valueOf hands the hook holder out and Object.assign replaces its useIt slot with a plain object, so the write lands with no warning; the checker must not prove a violation.
import { createStore } from "solid-js";

export function C1() {
  const [s1] = createStore({ v: 0, a: { v: 0 } });
  const useThing = () => ({ useIt: s1 });
  const thing = useThing();
  return (
    <div>
      <button data-act="b1" onClick={() => {
        Object.assign(thing.valueOf(), { useIt: { a: { v: 0 } } });
        thing.useIt.a.v = 7;
      }}>go</button>
    </div>
  );
}
