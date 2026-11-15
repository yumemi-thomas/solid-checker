import { createMemo, createRenderEffect } from "solid-js";
export function App() {
  const a = createMemo(async () => 1);
  createRenderEffect(() => a(), () => {});
  return <i>app</i>;
}
