/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createPermission } from "@solid-primitives/permission";
export default function App() {
  onSettled(() => { try { createPermission("camera"); } catch { /* Preserve structured diagnostic. */ } });
  return <p>ready</p>;
}
