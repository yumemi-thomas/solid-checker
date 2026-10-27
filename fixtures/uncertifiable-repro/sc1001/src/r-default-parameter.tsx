// ai-memory-ui scope-overview.tsx:57 (callers :105, :128): a helper whose parameter default reads
// a prop (`project = props.project`) is called from a callback prop and from a handler.
import { createSignal } from "solid-js";

function Body(props: { onOpenDoc: (path: string) => void }) {
  return <button onClick={() => props.onOpenDoc("a")}>Open</button>;
}

export function ScopeOverview(props: { project: string }) {
  const [last, setLast] = createSignal("");
  const openDoc = (path: string, project = props.project) => setLast(`${project}/${path}`);
  return (
    <div>
      <Body onOpenDoc={(path) => openDoc(path)} />
      <button onClick={() => openDoc("readme")}>Readme</button>
      <p>{last()}</p>
    </div>
  );
}
