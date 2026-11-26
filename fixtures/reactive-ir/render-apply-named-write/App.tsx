import { createRenderEffect, createSignal } from "solid-js";

// ADR 1227: the apply is a `const` handed by name, its one use. A render effect's first apply runs at its call under the
// component's owner, so the write throws REACTIVE_WRITE_IN_OWNED_SCOPE on rc.21. A violation.
export function App() {
  const [, setW] = createSignal(0);
  const apply = (v: number) => {
    setW(v);
  };
  createRenderEffect(() => 1, apply);
  return <div />;
}
