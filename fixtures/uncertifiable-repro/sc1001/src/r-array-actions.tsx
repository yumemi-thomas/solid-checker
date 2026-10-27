// app-game browser-atlas App.tsx:1317 (`visibleTreeTitle()` read through a helper in
// `printVisibleTree`, which is the `run` of an object in a returned array).
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
  const [source, setSource] = createSignal("tree");
  const visibleTitle = () => `Visible ${source()}`;
  function printTree(): void {
    console.log(visibleTitle());
  }
  const globalActions = (): Action[] => [{ label: "Print", run: printTree }];
  return (
    <div>
      <input value={source()} onInput={(event) => setSource(event.currentTarget.value)} />
      <Menu actions={globalActions()} />
    </div>
  );
}
