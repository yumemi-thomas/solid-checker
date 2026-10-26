// app-game ui-components-examples solid-props-proxy/body-modes.tsx:52,69-75 (`exit`) and paint input-visualizer/shortcut-settings.tsx:228: a child
// hands its callback prop to a package function (`onCleanup`) and also calls it inside another package callback.
import { createSignal, onCleanup, Show } from "solid-js";

function PickMode(props: { exit: () => void }) {
  onCleanup(props.exit);
  onCleanup(() => props.exit());
  return <p>picking</p>;
}

export function Modes() {
  const [pick, setPick] = createSignal(true);
  return (
    <Show when={pick()}>
      <PickMode exit={() => setPick(false)} />
    </Show>
  );
}
