// kui feature-topics TopicsRoute.tsx:1045 (`plan` read by PlannedActionDialog) and app-game typegpu-draw-app-example index.tsx:314: a callback that
// reads a component prop is handed to a child that calls it inside a `createEffect` apply function, a package callback.
import { createEffect } from "solid-js";

function Watcher(props: { source: string; onChange: (value: string) => void }) {
  createEffect(
    () => props.source,
    (value) => props.onChange(value),
  );
  return null;
}

export function Page(props: { clusterId: string }) {
  return <Watcher source="a" onChange={(value) => console.log(props.clusterId, value)} />;
}
