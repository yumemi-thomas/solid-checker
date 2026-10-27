// app-game browser-atlas App.tsx:2391 and compass-ui Bridge.tsx:321: a callback that writes is a
// member of an object handed to `Array.prototype.push`, collecting the actions of a menu.
import { createSignal, For } from "solid-js";

type Action = { label: string; run: () => void };

function Menu(props: { actions: Action[] }) {
  return (
    <ul>
      <For each={props.actions}>{(action) => <li onClick={() => action.run()}>{action.label}</li>}</For>
    </ul>
  );
}

export function Explorer() {
  const [clipboard, setClipboard] = createSignal("");
  const actions: Action[] = [];
  for (const node of ["a", "b"]) {
    actions.push({ label: `Cut ${node}`, run: () => setClipboard(node) });
  }
  return (
    <div>
      <Menu actions={actions} />
      <p>{clipboard()}</p>
    </div>
  );
}
