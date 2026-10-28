// ADR 0482 control: the same write inside its own setter, called through the member, lands. `#w` shows 7.
import { createStore } from "solid-js";

export default function App() {
  const [state, setState] = createStore({ v: 0 });
  const useState = () => ({ state, setState });
  const holder = useState();
  return (
    <div>
      <button id="b" onClick={() => holder.setState(() => { holder.state.v = 7; })}>set</button>
      <span id="w">{state.v}</span>
    </div>
  );
}
