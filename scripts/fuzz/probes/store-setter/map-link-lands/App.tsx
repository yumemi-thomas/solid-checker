// ADR 0491 negative: a link holding a Map is served raw (isWrappable refuses its [object Map] tag), so the write lands with no warning; the checker must not prove a violation.
import { createStore } from "solid-js";

export function C1() {
  const [s1] = createStore({ v: 0, a: Object.assign(new Map<string, number>(), { v: 0 }) });
  return (
    <div>
      <button data-act="b1" onClick={() => {
        s1.a.v = 7;
      }}>go</button>
    </div>
  );
}
