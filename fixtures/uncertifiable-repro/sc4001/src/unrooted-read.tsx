// oscartbeaumont-website routes/addy/index.tsx:95-101: a component renders an async memo with no Loading boundary,
// and is an exported default no tag renders (the project has a root; a route file is not traced to it).
import { createMemo, For } from "solid-js";

async function loadHashes(): Promise<string[]> {
  return ["a", "b"];
}

export default function Page() {
  const imageHashes = createMemo(() => loadHashes());
  return <For each={imageHashes()}>{(hash) => <img src={hash} />}</For>;
}
