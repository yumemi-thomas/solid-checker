import { createSignal, createSignal as cs, Loading } from "solid-js";
const over: { sync?: boolean } = { sync: false };
export function App() {
  try {
  const [m] = createSignal(async () => 1, { sync: true }); void m;
  } catch (e) {}
  return <i>app</i>;
}
