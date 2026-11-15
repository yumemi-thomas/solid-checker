import { createTrackedEffect, createSignal as cs, Loading } from "solid-js";
const over: { sync?: boolean } = { sync: false };
export function App() {
  try {
  createTrackedEffect(async () => { await 0; }, { sync: true });
  } catch (e) {}
  return <i>app</i>;
}
