import { createRenderEffect, createSignal } from "solid-js";

// ADR 1227: `defer` skips the first apply (rc.21 `dist/dev.js:2628-2629`): no first run under the owner. Uncertifiable.
export function App() {
  const [, setW] = createSignal(0);
  const apply = (v: number) => {
    setW(v);
  };
  createRenderEffect(() => 1, apply, { defer: true });
  return <div />;
}
