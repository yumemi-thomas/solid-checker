import { runWithOwner } from "solid-js";
import { createPointerPosition } from "@solid-primitives/pointer";
export default function App() {
  runWithOwner(null, () => createPointerPosition()); // Expected NO_OWNER_EFFECT.
  return <p>ready</p>;
}
