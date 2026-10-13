/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createWorker } from "@solid-primitives/workers";
export default function App() {
  onSettled(() => { try { createWorker({ add: (a: number, b: number) => a + b }); } catch { /* Preserve cleanup diagnostic. Worker dies with the isolated page. */ } });
  return <p>ready</p>;
}
