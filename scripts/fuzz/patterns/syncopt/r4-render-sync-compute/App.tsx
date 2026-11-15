import { createRenderEffect } from "solid-js";
export function App() {
  createRenderEffect(() => 1, () => {});
  return <i>app</i>;
}
