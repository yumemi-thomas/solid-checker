import { createResizeObserver } from "@solid-primitives/resize-observer";
createResizeObserver(document.body, () => {});
export default function App() { return <p>observer</p>; }
