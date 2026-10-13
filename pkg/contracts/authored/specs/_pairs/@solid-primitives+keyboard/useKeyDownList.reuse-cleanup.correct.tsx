/** @jsxImportSource @solidjs/web */
import { useKeyDownList } from "@solid-primitives/keyboard";
export default function App() {
  useKeyDownList();
  useKeyDownList();
  return <p>ready</p>;
}
