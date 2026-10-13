import { onSettled } from "solid-js";
import { createReducer } from "@solid-primitives/memo";
export default function App() {
  onSettled(() => { try { createReducer((value: () => number) => value, () => 1); } catch { /* Keep the package diagnostic and allow mount. */ } });
  return <p>ready</p>;
}
