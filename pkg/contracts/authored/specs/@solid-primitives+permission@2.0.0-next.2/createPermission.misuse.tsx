/** @jsxImportSource @solidjs/web */
import { createPermission } from "@solid-primitives/permission";
export default function App() {
  const camera = createPermission("camera");
  const frozen = camera();
  return <p>{String(frozen)}</p>;
}
