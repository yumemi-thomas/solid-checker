/** @jsxImportSource @solidjs/web */
import { keyArray } from "@solid-primitives/keyed";
export default function App() {
  keyArray(() => [1, 2], item => item, value => value());
  return <p>ready</p>;
}
