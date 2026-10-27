// solid-groove EditorView.tsx:186-188 (callers :486, :514) with SamplerPanel.tsx:93: a helper that writes is called from a literal
// handler and from a callback prop that two components forward before a child reads it in `<Show when>`; that second entry is not a
// delivery, so no caller places it.
import { createSignal, Show } from "solid-js";

function Picker(props: { onBrowse?: () => void }) {
  return (
    <Show when={props.onBrowse}>
      <button onClick={() => props.onBrowse?.()}>Browse</button>
    </Show>
  );
}

function Panel(props: { onBrowse?: () => void }) {
  return <Picker onBrowse={props.onBrowse} />;
}

function Shelf(props: { onBrowse?: () => void }) {
  return <Panel onBrowse={props.onBrowse} />;
}

export function Editor() {
  const [open, setOpen] = createSignal(false);
  function openLibrary() {
    setOpen(true);
  }
  return (
    <div>
      <button onClick={() => openLibrary()}>Loop</button>
      <Shelf onBrowse={() => openLibrary()} />
      <p>{open() ? "open" : ""}</p>
    </div>
  );
}
