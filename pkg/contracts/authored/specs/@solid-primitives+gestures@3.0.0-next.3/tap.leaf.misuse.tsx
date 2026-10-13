/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { tap } from "@solid-primitives/gestures";
export default function App() {
  onSettled(() => { try { tap({ callback: position => { console.log(position.x, position.y); } }); } catch { /* Preserve the structured leaf diagnostic. */ } });
  return <p>ready</p>;
}
