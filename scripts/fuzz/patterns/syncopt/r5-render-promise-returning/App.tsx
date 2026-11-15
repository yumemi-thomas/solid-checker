import { createRenderEffect } from "solid-js";
export function App() {
  createRenderEffect(() => Promise.resolve(1), () => {});
  return <i>app</i>;
}
