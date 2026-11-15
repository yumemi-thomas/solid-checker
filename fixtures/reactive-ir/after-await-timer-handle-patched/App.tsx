import { createSignal } from "solid-js";

const [flag, setFlag] = createSignal(false);
const pause = () => Promise.resolve();

// ADR 0984's twin of `after-await-timer-handle`'s `Cleared`: the program writes a built-in (`String.prototype.concat` is the project's function now), so no standard call is proven to run
// no project code, a `clearTimeout(timer)` included: the same shape keeps its uncertifiable row.
String.prototype.concat = function () {
  return String(this);
};

export function Cleared() {
  let timer: ReturnType<typeof setTimeout> | undefined;
  void pause().then(() => {
    clearTimeout(timer);
    timer = setTimeout(() => setFlag(false), 1500);
  });
  return <p>x</p>;
}
