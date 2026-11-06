import { createSignal } from "solid-js";
export default function useCount() {
  const [count, setCount] = createSignal(1);
  return { count, setCount };
}
