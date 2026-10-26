// inferay InlineDirectoryPicker/SelectedDirectoryChip.tsx:28 (callback from index.tsx:172, :262): the child binds a callback prop,
// `onClick={props.onRemove.bind(null, props.path)}`, so the walk meets a member of the prop instead of a call.
import { createSignal } from "solid-js";

function Chip(props: { path: string; onRemove: (path: string) => void }) {
  return <button onClick={props.onRemove.bind(null, "/a")}>{props.path}</button>;
}

export function Picker() {
  const [selected, setSelected] = createSignal(["/a", "/b"]);
  const toggle = (path: string) => {
    setSelected(selected().filter((item) => item !== path));
  };
  return <Chip path={selected()[0] ?? ""} onRemove={toggle} />;
}
