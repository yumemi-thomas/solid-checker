import { createResizeObserver } from "@solid-primitives/resize-observer";
export default function App() {
  createResizeObserver(document.body, () => {});
  return <p>observer</p>;
}

