/** @jsxImportSource @solidjs/web */
import { createSwitchTransition } from "@solid-primitives/transition-group";
export default function App() {
  const list = createSwitchTransition(() => 1, {});
  return <p>{String(list())}</p>;
}
