import { createRenderEffect, createSignal } from "solid-js";

// ADR 1227: the named apply is also a click handler, where the write is legal: the name has two uses, so no proof
// places this site. Uncertifiable.
export function App() {
  const [, setW] = createSignal(0);
  const apply = () => {
    setW(1);
  };
  createRenderEffect(() => 1, apply);
  return <button onClick={apply} />;
}
