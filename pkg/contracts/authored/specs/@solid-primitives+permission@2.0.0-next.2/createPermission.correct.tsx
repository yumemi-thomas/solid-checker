/** @jsxImportSource @solidjs/web */
import { createPermission } from "@solid-primitives/permission";
export default function App() {
  const camera = createPermission("camera");
  return <p>{String(camera())}</p>;
}
