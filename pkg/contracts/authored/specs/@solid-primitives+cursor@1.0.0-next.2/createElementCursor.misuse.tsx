import { createElementCursor } from "@solid-primitives/cursor";
createElementCursor(document.body, "pointer"); // Expected NO_OWNER_EFFECT.
export default function App() { return <p>ready</p>; }
