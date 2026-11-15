import { createEffect, createSignal as cs, Loading } from "solid-js";
const over: { sync?: boolean } = { sync: false };
export function App() {
  try {
  // @ts-ignore
  createEffect(async () => 1, () => {}, { sync: true, sync: false });
  } catch (e) {}
  return <i>app</i>;
}
