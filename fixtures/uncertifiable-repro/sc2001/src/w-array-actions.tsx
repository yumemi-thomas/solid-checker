// app-game browser-atlas App.tsx:1221-1223 with :2477: a function that writes is the `run` of an
// object in an array literal a function returns (a context menu's actions).
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
  const [searchOpen, setSearchOpen] = createSignal(false);
  function openSearch(): void {
    setSearchOpen(true);
  }
  const globalActions = (): Action[] => [{ label: "Find", run: openSearch }];
  return (
    <div>
      <Menu actions={globalActions()} />
      <p>{searchOpen() ? "searching" : ""}</p>
    </div>
  );
}
