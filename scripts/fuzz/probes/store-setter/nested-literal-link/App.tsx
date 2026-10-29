// ADR 0491 (from the misuse matrix): a write below a literal link of a store nothing reaches raw is dropped (STORE_WRITE_OUTSIDE_SETTER).
import { createSignal, createStore } from "solid-js";

function h1(cb: () => unknown) {
  cb();
  return 1;
}

export function C1() {
  const [s1, setS1] = createStore({ v: 0, a: { v: 0 }, items: [0] });
  return (
    <div>
      <button data-act="b1" onClick={() => {
        h1(() => {
          s1.items.push(1);
        });
      }}>go</button>
    </div>
  );
}
