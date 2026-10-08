/** @jsxImportSource @solidjs/web */
import { createConnectivitySignal } from "@solid-primitives/connectivity";
import { sharedConfig } from "solid-js/internal";
export default function App() {
  const value = (() => {
    const previous = sharedConfig.hydrating;
    sharedConfig.hydrating = true;
    try { return createConnectivitySignal(); }
    finally { sharedConfig.hydrating = previous; }
  })();
  const frozen = value();
  return <p>{String(frozen)}</p>;
}
