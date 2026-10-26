// Clean. Mirrors jar-hell-web web/src/components/package/BadgesDialog.tsx:70: the handler is a thunk, so
// the prop is read when the click happens; `f.bind(null, props.x)` is SC1001 (read-in-handler-bind).
import { createSignal } from "solid-js";

function Result(props: { path: string; onSelect: (path: string) => void }) {
  return (
    <button type="button" onClick={() => props.onSelect(props.path)}>
      pick
    </button>
  );
}

export function App() {
  const [path] = createSignal("/tmp");
  return <Result path={path()} onSelect={(next) => console.log(next)} />;
}
