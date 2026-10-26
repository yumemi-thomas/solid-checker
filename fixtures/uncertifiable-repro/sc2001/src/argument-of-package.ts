// inferay useCommitGraphState.tsx:83-85,279: a handler stored in an object that is an argument of merge.
import { createSignal, merge } from "solid-js";

export function createHover(props: { rows: string[] }) {
  const [hovered, setHovered] = createSignal<string | null>(null);
  const handleRowHover = (id: string | null) => {
    setHovered(id);
  };
  return merge(props, { hovered, handleRowHover });
}
