import { createMemo, createSignal } from "solid-js";

// The twin of `ending-by-code-other-rule`: the sites whose own rule's row stands. A read answered by SC1001 and a write answered by SC2001 are ended by the finding of the rule
// that claims their code, so no `*-unaccounted` row stands beside them.
export function OwnRead() {
  const [count] = createSignal(0);
  const value = count();
  return <p>{value}</p>;
}

export function OwnWrite() {
  const [count, setCount] = createSignal(0);
  const double = createMemo(() => {
    setCount(1);
    return count() * 2;
  });
  return <p>{double()}</p>;
}
