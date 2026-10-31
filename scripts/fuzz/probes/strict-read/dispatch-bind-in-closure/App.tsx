// ADR 0517: a closure called in the body reads through a bound accessor (STRICT_READ_UNTRACKED).
import { createSignal } from "solid-js";

export default function App() {
  const [r] = createSignal(3);
  const read = () => {
    r.bind(null)();
  };
  read();
  return <span id="w">{r()}</span>;
}
