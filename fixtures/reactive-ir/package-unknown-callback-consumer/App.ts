import { createSignal } from "solid-js";
import { noCallback, runUnknown } from "partial-package";

const [count] = createSignal(0);

function readCount() {
  return count();
}

export function exercise() {
  // The reviewed contract explicitly says callback behavior is unknown.
  // Passing a callable therefore remains an uncertifiable proof obligation.
  runUnknown(readCount);

  // The same unknown callback domain is irrelevant when no callable argument
  // is present; this call must not receive a blanket contract finding.
  noCallback();
}
