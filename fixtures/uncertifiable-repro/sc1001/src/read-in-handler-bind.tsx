// Violation. Corpus: inferay modules/workspace/components/InlineDirectoryPicker/DirectoryResult.tsx:31,
// SelectedDirectoryChip.tsx:28, shared/ui/DropdownButton/DropdownCustomOption.tsx:40: a handler
// expression `f.bind(null, props.x)` reads the prop once, when the element is built.
import { createSignal } from "solid-js";

function Result(props: { path: string; onSelect: (path: string) => void }) {
  return (
    <button type="button" onClick={props.onSelect.bind(null, props.path)}>
      pick
    </button>
  );
}

export function App() {
  const [path] = createSignal("/tmp");
  return <Result path={path()} onSelect={(next) => console.log(next)} />;
}
