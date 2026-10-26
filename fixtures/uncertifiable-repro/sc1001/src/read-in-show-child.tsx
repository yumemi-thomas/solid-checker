// Violation. Corpus: ai-memory-ui src/screens/access.tsx:233 (and 17 more like it),
// solid-groove editor/LoopInfo.tsx:59, kui shell/features/FeatureGate.tsx:121: the child
// function of a non-keyed Show or Match is untracked, so reading its accessor in the body is a snapshot.
import { createSignal, Show } from "solid-js";

export function Gate() {
  const [dialog, setDialog] = createSignal<{ name: string } | null>({ name: "a" });
  return (
    <Show when={dialog()}>
      {(current) => {
        const name = current().name;
        return <button onClick={() => setDialog(null)}>{name}</button>;
      }}
    </Show>
  );
}
