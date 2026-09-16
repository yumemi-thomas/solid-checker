import { createSignal } from "solid-js";
import { runUnknown } from "partial-package";

const [other] = createSignal(1);

function readOther() {
  return other();
}

export function second() {
  // The same export, the same open domain, a second import site. The sentence
  // is identical, so this site travels in `relatedLocations` rather than as its
  // own finding.
  runUnknown(readOther);
}
