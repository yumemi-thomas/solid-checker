// ADR 0480: a setter reached through an object-literal arrow's slot, called in a memo, throws REACTIVE_WRITE_IN_OWNED_SCOPE.
import { createMemo, createSignal } from "solid-js";

export default function App() {
  const [count, setCount] = createSignal(0);
  const useCounter = () => ({ set: setCount });
  const { set } = useCounter();
  const m = createMemo(() => {
    set(1);
    return 1;
  });
  return <span id="w">{count()}{m()}</span>;
}
