// Clean. Mirrors compass-ui apps/ui/src/components/MessageStream.tsx:54: Show with a fallback for
// the conditional (the `cond() && <El/>` and ternary forms are SC8015, and-in-jsx).
import { createSignal, Show } from "solid-js";

function Stream(props: { emptyMessage: string }) {
  const [messages] = createSignal<string[]>([]);
  return (
    <Show when={messages().length > 0} fallback={<div>{props.emptyMessage}</div>}>
      <ul>{messages().length}</ul>
    </Show>
  );
}

export function App() {
  return <Stream emptyMessage="none" />;
}
