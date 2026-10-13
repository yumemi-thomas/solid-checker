import { sharedConfig } from "solid-js/internal";
import { createPrefersDark } from "@solid-primitives/media";
export default function App() {
  const prior = sharedConfig.hydrating;
  const dark = (() => {
    sharedConfig.hydrating = true;
    try { return createPrefersDark(); }
    finally { sharedConfig.hydrating = prior; }
  })();
  const current = dark();
  return <p>{String(current)}</p>;
}
