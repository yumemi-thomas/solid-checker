import * as S from "solid-js";
import { flush, onSettled } from "solid-js";
const bound = onSettled.bind(null);
let slot: typeof onSettled | undefined;

// Each runs where `run` runs: the body calls it (owned), and a click calls it
// later with no owner. The direct twin is `direct`, in the same places.
function run() {
  onSettled.bind(null)(() => () => {});
  onSettled.call(null, () => () => {});
  onSettled.apply(null, [() => () => {}]);
  (slot ||= onSettled)(() => () => {});
  bound(() => () => {});
  S.onSettled.call(null, () => () => {});
}
function direct() {
  onSettled(() => () => {});
}
export function Indirect() {
  run();
  direct();
  const a = document.createElement("button");
  a.onclick = run;
  const b = document.createElement("button");
  b.onclick = direct;
  return <i />;
}

// `flush` through `call`: ADR 0884's rows of `flush`, at the callee.
export function Flush() {
  setTimeout(() => flush.call(null, () => {}), 0);
  return <i />;
}

// A local function named `onSettled` is no primitive.
export function Shadow() {
  const onSettled = (_fn: () => () => void) => {};
  onSettled.bind(null)(() => () => {});
  onSettled.call(null, () => () => {});
  onSettled.apply(null, [() => () => {}]);
  return <i />;
}
