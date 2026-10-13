import { runWithOwner } from "solid-js";
import { createPointerPosition } from "@solid-primitives/pointer";
export default function App() {
  createPointerPosition();
  return <p>ready</p>;
}
