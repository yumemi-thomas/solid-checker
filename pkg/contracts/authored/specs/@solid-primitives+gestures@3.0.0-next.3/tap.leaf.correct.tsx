/** @jsxImportSource @solidjs/web */
import { tap } from "@solid-primitives/gestures";
export default function App() {
  tap({ callback: position => { console.log(position.x, position.y); } });
  return <p>ready</p>;
}
