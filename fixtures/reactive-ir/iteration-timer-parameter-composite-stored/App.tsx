import { createSignal } from "solid-js";
import { later, startBrowse } from "./session";

// ADR 1056, the stored twin: the composite callee also assigns `afterNextPaint` to a binding (`(slot = afterNextPaint)(…)`), an entry no call
// shows, so `resume` may be any function and the slots stay withdrawn: the write below is uncertifiable.
startBrowse();
later(function (this: unknown) {
  void this;
});

// REACTIVE_WRITE_IN_OWNED_SCOPE: a setter called in a component body, a violation where the signal's slots are exact.
export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
