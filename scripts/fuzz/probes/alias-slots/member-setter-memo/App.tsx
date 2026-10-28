// ADR 0481: a setter reached as a member of a hook's fresh result, called in a memo, throws REACTIVE_WRITE_IN_OWNED_SCOPE.
import { createMemo, createSignal } from "solid-js";

export default function App() {
  const [count, setCount] = createSignal(0);
  const useCounter = () => ({ set: setCount });
  const counter = useCounter();
  const m = createMemo(() => {
    counter.set(1);
    return 1;
  });
  return <span id="w">{count()}{m()}</span>;
}
