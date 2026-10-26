// oscartbeaumont-website routes/addy/index.tsx:95-101: a component renders an async memo with no Loading boundary,
// and the project never mounts it (no render or hydrate root, as in a route-file project).
import { createMemo, For } from "solid-js";

async function loadHashes(): Promise<string[]> {
  return ["a", "b"];
}

export default function Page() {
  const imageHashes = createMemo(() => loadHashes());
  return <For each={imageHashes()}>{(hash) => <img src={hash} />}</For>;
}
