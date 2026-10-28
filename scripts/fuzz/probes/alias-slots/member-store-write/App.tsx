// ADR 0482: a store written through a member of a hook's fresh result, outside its setter, is dropped (STORE_WRITE_OUTSIDE_SETTER).
import { createStore } from "solid-js";

export default function App() {
  const [state, setState] = createStore({ v: 0 });
  const useState = () => ({ state, setState });
  const holder = useState();
  holder.state.v = 7;
  return <span id="w">{state.v}</span>;
}
