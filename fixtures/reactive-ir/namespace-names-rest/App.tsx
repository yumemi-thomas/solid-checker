import * as S from "solid-js";

// The rest element holds every other export: the program may name every
// primitive, so the flush keeps its rows.
const { flush: flushD, ...rest } = S;

export function Rest() {
  void rest;
  return (
    <button onClick={() => {
      flushD();
    }}>go</button>
  );
}
