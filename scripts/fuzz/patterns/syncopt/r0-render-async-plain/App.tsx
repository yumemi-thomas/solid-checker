import { createEffect, createRenderEffect } from "solid-js";
export function App() {
  createRenderEffect(async () => 1, () => {});
  return <i>app</i>;
}
