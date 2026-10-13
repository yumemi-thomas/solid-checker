/** @jsxImportSource @solidjs/web */
import { createMediaQuery } from "@solid-primitives/media";
import { sharedConfig } from "solid-js/internal";
export default function App() {
  const value = (() => {
    const previous = sharedConfig.hydrating;
    sharedConfig.hydrating = true;
    try { return createMediaQuery("(min-width: 1px)", false); }
    finally { sharedConfig.hydrating = previous; }
  })();
  return <p>{String(value())}</p>;
}
