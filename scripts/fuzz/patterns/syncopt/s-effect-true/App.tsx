import { createEffect, createSignal as cs, Loading } from "solid-js";
const over: { sync?: boolean } = { sync: false };
export function App() {
  try {
  createEffect(async () => 1, () => {}, { sync: true });
  } catch (e) {}
  return <i>app</i>;
}
