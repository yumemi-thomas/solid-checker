import { createSignal } from "solid-js";
import { bump, readCount, readShared, useTicker } from "./helpers";

const [total] = createSignal(0);

// An export App calls: its body is this file's.
export function readTotal() {
  return total();
}

// The pinned rc.17 compiler refuses this file: `attr:` on an intrinsic element.
export function View() {
  return (
    <p>
      <span>{readCount()}{readShared()}{bump()}</span>
      <button onClick={() => useTicker()}>tick</button>
      <span attr:data-z="1" />
    </p>
  );
}
