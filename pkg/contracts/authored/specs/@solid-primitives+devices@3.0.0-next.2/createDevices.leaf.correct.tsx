/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createDevices } from "@solid-primitives/devices";
export default function App() {
  createDevices();
  onSettled(() => {});
  return <p>ready</p>;
}
