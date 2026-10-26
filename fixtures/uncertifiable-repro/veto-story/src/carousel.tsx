// kui kernel Avatar.tsx:77, JsonTree.tsx:254 (held only in the kernel project): handlers that read and write signals, in a
// project whose Storybook story `play` function dispatches an event on a receiver found by querySelector.
import { createSignal } from "solid-js";

export function Carousel(props: { pages: number }) {
  const [currentPage, setCurrentPage] = createSignal(0);
  return <button onClick={() => setCurrentPage(Math.min(props.pages - 1, currentPage() + 1))}>{currentPage() + 1}</button>;
}
