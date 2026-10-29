// ADR 0480 control: a write through a tuple holder's store slot inside its own setter lands (legal). `#w` shows the raw value 7.
import { createStore } from "solid-js";

export default function App() {
  const [store, setStore] = createStore({ v: 0 });
  const pair = [0, store] as const;
  const [, view] = pair;
  return (
    <div>
      <button id="b" onClick={() => setStore(() => { view.v = 7; })}>set</button>
      <span id="w">{store.v}</span>
    </div>
  );
}
