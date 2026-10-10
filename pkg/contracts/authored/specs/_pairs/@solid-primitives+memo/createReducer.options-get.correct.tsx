import { createSignal, untrack } from "solid-js";
import { createReducer } from "@solid-primitives/memo";
export default function App() {
  const [source] = createSignal("named");
  const [state] = createReducer((value: number) => value, 0, {
    get name() { return untrack(source); }
  });
  return <p>{state()}</p>;
}
