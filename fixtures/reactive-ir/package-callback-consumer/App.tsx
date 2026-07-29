import { createSignal } from "solid-js";
import { runDeferred, runMixed, runTracked } from "reactive-package";

const [count] = createSignal(0);

function readCount() {
  return count();
}

export function Good() {
  runTracked(readCount);
  runDeferred(readCount);
  return <div>good</div>;
}

export function Bad() {
  runMixed(readCount);
  return <div>bad</div>;
}
