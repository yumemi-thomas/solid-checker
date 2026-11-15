import { createOptimistic, createSignal as cs, Loading } from "solid-js";
const over: { sync?: boolean } = { sync: false };
export function App() {
  try {
  const [m] = createOptimistic(async () => 1, { sync: true, ...over }); void m;
  } catch (e) {}
  return <i>app</i>;
}
