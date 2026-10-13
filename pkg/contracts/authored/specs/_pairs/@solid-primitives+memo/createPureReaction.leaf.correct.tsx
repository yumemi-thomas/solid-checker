/** @jsxImportSource @solidjs/web */
import { createPureReaction } from "@solid-primitives/memo";
export default function App() {
  createPureReaction(() => {});
  return <p>ready</p>;
}
