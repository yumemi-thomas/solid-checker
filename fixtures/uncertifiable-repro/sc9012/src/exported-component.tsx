// kui shell/src/pages/SettingsPage.tsx:68 (rendered by a tag in app.tsx): an exported component calls a member of an
// object its props carry, in the attribute of a component element (`<Show when={...}>`).
import { Show } from "solid-js";

type Session = { brush: () => string };

export function Panel(props: { session: Session }) {
  return (
    <Show when={props.session.brush()}>
      <p>inactive</p>
    </Show>
  );
}
