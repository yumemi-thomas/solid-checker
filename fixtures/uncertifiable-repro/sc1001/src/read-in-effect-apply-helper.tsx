// Violation. Corpus: probus-hk src/routes/Plan.tsx:526 and RouteDetail.tsx:2214 (and 16 more
// like them): an effect's apply callback calls a helper that reads a signal; apply is untracked.
import { createEffect, createSignal } from "solid-js";

function Plan(props: { from: string }) {
  const [to, setTo] = createSignal("");
  const sync = (from: string) => {
    if (to() !== from) setTo(from);
  };
  createEffect(
    () => props.from,
    (from) => {
      sync(from);
    },
  );
  return <p>{to()}</p>;
}

export function App() {
  const [from] = createSignal("a");
  return <Plan from={from()} />;
}
