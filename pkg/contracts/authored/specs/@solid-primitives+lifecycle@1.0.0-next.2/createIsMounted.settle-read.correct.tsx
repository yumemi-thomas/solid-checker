/** @jsxImportSource @solidjs/web */
import { createIsMounted } from "@solid-primitives/lifecycle";
export default function App() {
  const value = createIsMounted();
  return <p>{String(value())}</p>;
}
