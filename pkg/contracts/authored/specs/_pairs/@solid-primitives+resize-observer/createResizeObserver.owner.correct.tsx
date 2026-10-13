import { createRoot } from "solid-js";
import { createResizeObserver } from "@solid-primitives/resize-observer";
createRoot(dispose => {
  createResizeObserver(document.body, () => {});
  dispose();
});
export default function App() { return <p>observer</p>; }
