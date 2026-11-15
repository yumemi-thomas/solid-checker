import { createEffect, createRenderEffect } from "solid-js";
export function App() {
  createEffect(async () => 1, () => {});
  return <i>app</i>;
}
