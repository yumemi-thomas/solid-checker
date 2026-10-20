import { onCleanup } from "solid-js";
function Kids(props: {onClick: () => void}) {props.onClick(); return <button>owned</button>;}
function handlers(cb: () => void) {return {onClick: () => cb()};}
export function App() {
  const h=handlers(() => {onCleanup(() => undefined);});
  return <Kids {...h}/>;
}
