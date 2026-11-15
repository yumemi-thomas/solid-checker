import { createSignal } from "solid-js";

const [flag, setFlag] = createSignal(false);
const pause = () => Promise.resolve();

// ADR 0984's twin of `after-await-timer-handle`'s `Cleared`, for the Node runtime target: a timer handle is a `Timeout` object there, whose `Symbol.toPrimitive` a program could replace
// on `Timeout.prototype` (the proof is the browser's alone), so the same shape keeps its uncertifiable row.
export function Cleared() {
  let timer: ReturnType<typeof setTimeout> | undefined;
  void pause().then(() => {
    clearTimeout(timer);
    timer = setTimeout(() => setFlag(false), 1500);
  });
  return <p>x</p>;
}
