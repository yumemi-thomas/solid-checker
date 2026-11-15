import { createMemo, Show } from "solid-js";
import { Portal } from "@solidjs/web";

// ADR 0992: `Portal` reads `props.children` in `createMemo(() => [startMarker, props.children])` under the owner `createOwner()` made, so the tracked `{text()}` of the panel is
// created under an owner: Chrome reports nothing and the checker is silent.
// (`mount` is its own case, `portal-mount-owner`.)
export function Tip() {
  const text = createMemo(() => "tip text");
  return (
    <Portal>
      <div role="tooltip">{text()}</div>
    </Portal>
  );
}

// The shape kui writes (Drawer, Dialog, Tooltip): a `Show` around a `Portal` holding the panel, whose own text is read in the portal's children.
export function Drawer(props: { title: string }) {
  const text = createMemo(() => "drawer body");
  return (
    <Show when={true}>
      <Portal>
        <div role="dialog">
          <h2>{props.title}</h2>
          <p>{text()}</p>
        </div>
      </Portal>
    </Show>
  );
}
