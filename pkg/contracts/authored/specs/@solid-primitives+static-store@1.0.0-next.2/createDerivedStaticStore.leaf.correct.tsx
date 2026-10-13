import { onSettled } from "solid-js";
import { createDerivedStaticStore } from "@solid-primitives/static-store";
export default function App() {
  createDerivedStaticStore(() => ({ value: 0 }));
  onSettled(() => {});
  return <p>ready</p>;
}
