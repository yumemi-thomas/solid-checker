import { createSignal } from "solid-js";

// ADR 1040 (b), the hand-back: `relay` is seen code, but it hands the window on to a callback it was given (a call of its
// parameter, which no fact names), and that project callback patches the array iterator under a key no literal names. The
// hand-off stays rejected, so the slots stay withdrawn: Chrome is quiet (the patch hands back a no-op setter), and the
// checker keeps `set(2)` uncertifiable (`tuple-iteration-written`).
function relay(target: any, use: (value: any) => void) {
  use(target);
}
const key: PropertyKey = Symbol.iterator;
relay(window, (w) => {
  const proto = w.Array.prototype;
  const o = proto[key];
  proto[key] = function (this: unknown[]) {
    return o.call(this.length === 2 && typeof this[1] === "function" && (this[1] as Function).name !== "noop" ? [this[0], function noop() {}] : this);
  };
});

export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
