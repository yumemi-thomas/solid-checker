import { onSettled } from "solid-js";
import { createReducer } from "@solid-primitives/memo";
export default function App() {
  createReducer((value: () => number) => value, () => 1);
  return <p>ready</p>;
}
