import { toEffect } from "@solid-primitives/event-bus";
toEffect<string>(() => {}); // Expected NO_OWNER_EFFECT.
export default function App() { return <p>ready</p>; }
