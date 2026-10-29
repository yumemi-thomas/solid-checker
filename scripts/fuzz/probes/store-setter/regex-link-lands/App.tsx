// ADR 0491 review negative: a regular expression link is served raw (isWrappable refuses [object RegExp]), so the write lands with no warning; the checker must not prove a violation.
import { createStore } from "solid-js";

export function C1() {
  const [s1] = createStore({ v: 0, r: /x/g });
  return (
    <div>
      <button data-act="b1" onClick={() => {
        s1.r.lastIndex = 5;
      }}>go</button>
    </div>
  );
}
