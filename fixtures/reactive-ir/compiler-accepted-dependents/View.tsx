import { createSignal } from "solid-js";
import { bump, readCount, readShared, useTicker } from "./helpers";

const [total] = createSignal(0);

// An export App calls: its body is this file's.
export function readTotal() {
  return total();
}

// The compiler accepts this file: the twin of the refused View.tsx.
export function View() {
  return (
    <p>
      <span>{readCount()}{readShared()}{bump()}</span>
      <button onClick={() => useTicker()}>tick</button>
      <span data-z="1" />
    </p>
  );
}
