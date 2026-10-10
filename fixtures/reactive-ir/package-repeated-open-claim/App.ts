import { createSignal } from "solid-js";
import { runUnknown } from "partial-package";

const [count] = createSignal(0);

function readCount() {
  return count();
}

export function first() {
  // The reviewed contract marks this export's callback domain open, so passing
  // a callable is an uncertifiable proof obligation -- once.
  runUnknown(readCount);
}
