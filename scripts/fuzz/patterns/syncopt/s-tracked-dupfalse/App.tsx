import { createTrackedEffect, createSignal as cs, Loading } from "solid-js";
const over: { sync?: boolean } = { sync: false };
export function App() {
  try {
  // @ts-ignore
  createTrackedEffect(async () => { await 0; }, { sync: true, sync: false });
  } catch (e) {}
  return <i>app</i>;
}
