/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createPermission } from "@solid-primitives/permission";
export default function App() {
  createPermission("camera");
  onSettled(() => {});
  return <p>ready</p>;
}
