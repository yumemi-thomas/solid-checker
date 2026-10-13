/** @jsxImportSource @solidjs/web */
import { createIsMounted } from "@solid-primitives/lifecycle";
export default function App() {
  const value = createIsMounted();
  const frozen = value();
  return <p>{String(frozen)}</p>;
}
