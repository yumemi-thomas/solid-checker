// inferay CheckpointMarker.tsx:52, TreeNodeRow.tsx:175 (also ChangesPanelHeader.tsx:29, ExplorerTreeRow.tsx:163):
// a handler written on an element inside a conditional or logical JSX expression reads a prop and an accessor;
// the same handler written directly in the returned element (the first button) is placed.
import { createSignal } from "solid-js";

export function Checkpoint(props: { id: string; reverted: boolean; onRevert: (id: string) => void }) {
  const [expanded, setExpanded] = createSignal(false);
  return (
    <div>
      <button type="button" onClick={() => setExpanded(!expanded())}>
        Details
      </button>
      {!props.reverted ? (
        <button type="button" onClick={() => props.onRevert(props.id)}>
          Undo
        </button>
      ) : (
        <span>reverted</span>
      )}
      {props.id && (
        <button type="button" onClick={() => setExpanded(!expanded())}>
          Collapse
        </button>
      )}
    </div>
  );
}
