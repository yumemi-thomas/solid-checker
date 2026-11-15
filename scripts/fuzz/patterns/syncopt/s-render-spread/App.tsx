import { createRenderEffect, createSignal as cs, Loading } from "solid-js";
const over: { sync?: boolean } = { sync: false };
export function App() {
  try {
  createRenderEffect(async () => 1, () => {}, { sync: true, ...over });
  } catch (e) {}
  return <i>app</i>;
}
