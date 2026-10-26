// Clean. Mirrors inferay src/modules/repository/components/operations/ChatDiffPanel/
// RepositoryOperationBar.tsx:24-37: For with a key function hands the callback an accessor, read in JSX.
import { createSignal, For } from "solid-js";

export function Bar() {
  const [actions] = createSignal([{ label: "pull", disabled: false }]);
  return (
    <For each={actions()} keyed={(row) => row.label}>
      {(action) => (
        <button type="button" disabled={action().disabled} onClick={() => console.log(action().label)}>
          {action().label}
        </button>
      )}
    </For>
  );
}
