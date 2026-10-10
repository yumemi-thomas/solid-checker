import { createBodyCursor } from "@solid-primitives/cursor";
createBodyCursor(() => "pointer"); // Expected NO_OWNER_EFFECT.
export default function App() { return <p>ready</p>; }
