import { createSignal } from "solid-js";
import { asArray } from "@solid-primitives/utils";
export default function App() {
  const [source] = createSignal(1);
  const array = asArray(source);
  const value = array[0]();
  return <p>{String(value)}</p>;
}
