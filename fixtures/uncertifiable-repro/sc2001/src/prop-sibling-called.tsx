// app-game spector/solid/spector-result-view.tsx:123 (CaptureHistory at :317): a component calls another prop as a method
// (`props.onLoad?.(...)`) whose value the caller gives as a member read (`props.onAdd`), which the proof cannot show is free of
// `this`, so none of the component's callback props is placed.
import { createSignal } from "solid-js";

function History(props: { onSelect: (id: string) => void; onLoad?: (id: string) => void }) {
  return (
    <div>
      <button onClick={() => props.onSelect("a")}>Pick</button>
      <button onClick={() => props.onLoad?.("b")}>Load</button>
    </div>
  );
}

export function Viewer(props: { onAdd?: (id: string) => void }) {
  const [tab, setTab] = createSignal("");
  function selectCapture(id: string): void {
    setTab(id);
  }
  return <History onSelect={selectCapture} onLoad={props.onAdd} />;
}
