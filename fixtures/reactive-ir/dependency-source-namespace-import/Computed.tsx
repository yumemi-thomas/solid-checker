import { createSignal } from "solid-js";
import * as Computed from "dep-ns/widget";

declare const key: "readNow";
// A computed member names nothing statically.
export function ComputedMember() {
  const [count] = createSignal(0);
  const now = Computed[key](() => count());
  return <Computed.Root>{now}</Computed.Root>;
}
