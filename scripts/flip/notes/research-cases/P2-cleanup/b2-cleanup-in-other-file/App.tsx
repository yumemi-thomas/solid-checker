import { createRoot } from "solid-js";
import { useX } from "./h";
export function App() {
  const dispose = createRoot((d) => { useX(); return d; });
  dispose();
  return <p />;
}
