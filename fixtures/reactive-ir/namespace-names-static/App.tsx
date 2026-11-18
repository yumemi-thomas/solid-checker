import * as S from "solid-js";

const { flush: flushD } = S;

// The namespace's every reference is a static member or a static slot, so the
// program names `createSignal` and `flush` and no primitive that steps an
// action, defers an apply or opens a leaf scope: both flushes are clean.
export function Member() {
  const [count] = S.createSignal(0);
  return (
    <button onClick={() => {
      S.flush();
    }}>{count()}</button>
  );
}

export function Slot() {
  return (
    <button onClick={() => {
      flushD();
    }}>go</button>
  );
}
