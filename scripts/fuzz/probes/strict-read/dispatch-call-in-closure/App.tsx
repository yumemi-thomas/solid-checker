// ADR 0517: a closure called in the body reads through Function.prototype.call (STRICT_READ_UNTRACKED).
import { createSignal } from "solid-js";

export default function App() {
  const [r] = createSignal(3);
  const read = () => {
    r.call(null);
  };
  read();
  return <span id="w">{r()}</span>;
}
