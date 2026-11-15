import { createEffect, createRenderEffect } from "solid-js";
export function App() {
  createRenderEffect(async () => 1, () => {}, { sync: false });
  return <i>app</i>;
}
