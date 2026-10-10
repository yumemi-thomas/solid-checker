/** @jsxImportSource @solidjs/web */
import { createPageVisibility } from "@solid-primitives/page-utilities";
import { sharedConfig } from "solid-js/internal";
export default function App() {
  const value = (() => {
    const previous = sharedConfig.hydrating;
    sharedConfig.hydrating = true;
    try { return createPageVisibility(); }
    finally { sharedConfig.hydrating = previous; }
  })();
  return <p>{String(value())}</p>;
}
