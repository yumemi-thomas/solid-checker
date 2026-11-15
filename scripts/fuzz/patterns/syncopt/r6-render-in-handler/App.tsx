import { createRenderEffect } from "solid-js";
export function App() {
  return <button onClick={() => { createRenderEffect(async () => 1, () => {}); }}>go</button>;
}
