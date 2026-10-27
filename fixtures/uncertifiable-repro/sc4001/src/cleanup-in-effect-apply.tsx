// Violation. Corpus: error-menu-web web/src/components/ChangeList.tsx:97, kui feature-messages/
// MessagesTab.tsx:218: onCleanup in an effect's apply callback has no owner; apply should return it.
import { createEffect, createSignal, onCleanup } from "solid-js";

function ChangeList(props: { count: number }) {
  const [rows, setRows] = createSignal(0);
  createEffect(
    () => props.count,
    (count) => {
      const timer = setInterval(() => setRows(count), 1000);
      onCleanup(() => clearInterval(timer));
    },
  );
  return <p>{rows()}</p>;
}

export function App() {
  const [count] = createSignal(1);
  return <ChangeList count={count()} />;
}
