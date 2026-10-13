import { createSignal } from "solid-js";
import { readAfterAwait, readArgument, readDefault, readLater, readNow, readThroughHelper } from "./helpers";

// ---- Proven: the helper's own body reads while the call runs.

export function CallsReadNow() {
  const value = readNow();
  return <main>{value}</main>;
}
export function CallsReadArgument() {
  const [local] = createSignal(1);
  const value = readArgument(local);
  return <main>{value}</main>;
}

// ---- Not proven: the read may run elsewhere, or later.

export function CallsReadLater() {
  const later = readLater();
  return <main>{later()}</main>;
}
export function CallsReadAfterAwait() {
  void readAfterAwait();
  return <main>a</main>;
}
export function CallsReadDefault() {
  const value = readDefault();
  return <main>{value}</main>;
}
export function CallsReadThroughHelper() {
  const value = readThroughHelper();
  return <main>{value}</main>;
}
export function CallsInJsxProp() {
  return <Show value={readNow()} />;
}
function Show(props: { value: number }) {
  return <main>{props.value}</main>;
}
