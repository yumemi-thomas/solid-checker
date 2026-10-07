import { createTimer } from "@solid-primitives/timer";
createTimer(() => {}, () => 30_000, setInterval);
export default function App() { return <p>timer</p>; }

