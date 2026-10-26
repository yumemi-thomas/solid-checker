// app-game gpu-text-rendering's viewer controls: handlers that read and write
// signals, certified in a project with no unexcluded dispatch.
import { createSignal, Show } from "solid-js";

export function Viewer() {
  const [zoom, setZoom] = createSignal(1);
  const [panel, setPanel] = createSignal(false);
  return (
    <div>
      <button onClick={() => setZoom(zoom() * 2)}>Zoom</button>
      <button onClick={() => setPanel(!panel())}>Panel</button>
      <Show when={panel()}>
        <p>{zoom()}</p>
      </Show>
    </div>
  );
}
