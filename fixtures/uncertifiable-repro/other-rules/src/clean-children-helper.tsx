// Clean. Mirrors skyjtx-website apps/skyjt-website/src/components/Card/Card.tsx:18-24 (and spookysoftware-www
// root.tsx:15): children() resolves slot props once; the resolved accessor is read in Show conditions.
import { children, Show } from "solid-js";
import type { JSX } from "@solidjs/web";

function Card(props: { media?: JSX.Element; actions?: JSX.Element; children?: JSX.Element }) {
  const media = children(() => props.media);
  const actions = children(() => props.actions);
  return (
    <div>
      <Show when={media()}>
        <figure>{media()}</figure>
      </Show>
      {props.children}
      <Show when={actions()}>
        <footer>{actions()}</footer>
      </Show>
    </div>
  );
}

export function App() {
  return <Card media={<img alt="m" />} actions={<button type="button">go</button>}>body</Card>;
}
