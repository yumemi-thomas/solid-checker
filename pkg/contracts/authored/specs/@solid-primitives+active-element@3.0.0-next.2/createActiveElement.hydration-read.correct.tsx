/** @jsxImportSource @solidjs/web */
import { createActiveElement } from "@solid-primitives/active-element";
import { sharedConfig } from "solid-js/internal";
export default function App() {
  const value = (() => {
    const previous = sharedConfig.hydrating;
    sharedConfig.hydrating = true;
    try { return createActiveElement(); }
    finally { sharedConfig.hydrating = previous; }
  })();
  return <p>{String(value())}</p>;
}
