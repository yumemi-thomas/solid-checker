// app-game browser-atlas App.tsx:2391 (`cutContextNode(node)`): a callback that reads a signal is a
// member of an object handed to `Array.prototype.push`, collecting a menu's actions.
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
  const [title, setTitle] = createSignal("tree");
  const actions: Action[] = [];
  for (const node of ["a", "b"]) {
    actions.push({ label: `Cut ${node}`, run: () => console.log(title(), node) });
  }
  return (
    <div>
      <input value={title()} onInput={(event) => setTitle(event.currentTarget.value)} />
      <Menu actions={actions} />
    </div>
  );
}
