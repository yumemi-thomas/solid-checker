// Violation. Corpus: probus-hk src/routes/Plan.tsx:528 and RailDiagram.tsx:349 (and 17 more
// like them): an effect's apply callback reads a signal or memo directly; apply is untracked.
import { createEffect, createMemo, createSignal } from "solid-js";

function Plan(props: { from: string }) {
  const [to, setTo] = createSignal("");
  const label = createMemo(() => props.from.toUpperCase());
  createEffect(
    () => props.from,
    (from) => {
      if (to() !== from) setTo(from);
      console.log(label());
    },
  );
  return <p>{to()}</p>;
}

export function App() {
  const [from] = createSignal("a");
  return <Plan from={from()} />;
}
