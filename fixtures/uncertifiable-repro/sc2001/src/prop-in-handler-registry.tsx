// solid-groove components/ConfirmDialog.tsx:42-52 (used at instrument/InstrumentKindPicker.tsx:123): a child calls its callback prop inside an
// object member of a thunk that a project function (`useShortcuts`) consumes.
import { createSignal, Show } from "solid-js";

type Handlers = Record<string, { run: () => void; isEnabled: () => boolean }>;

function useShortcuts(options: { handlers: () => Handlers }): () => string[] {
  return () => Object.keys(options.handlers());
}

function ConfirmDialog(props: { busy?: boolean; onCancel: () => void }) {
  useShortcuts({
    handlers: () => ({
      "view.close": { run: () => props.onCancel(), isEnabled: () => props.busy !== true },
    }),
  });
  return <button onClick={() => props.onCancel()}>Cancel</button>;
}

export function Picker() {
  const [pending, setPending] = createSignal<string | null>("drum");
  return (
    <Show when={pending()}>
      <ConfirmDialog onCancel={() => setPending(null)} />
    </Show>
  );
}
