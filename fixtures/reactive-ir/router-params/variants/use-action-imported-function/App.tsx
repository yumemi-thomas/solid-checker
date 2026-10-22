import { Show } from "solid-js";
import { useAction, useParams } from "@solidjs/router";
import { save } from "./actions";

declare function lookup(value: unknown): number;
function fetchThing(id: string) {
  return lookup(id);
}
function Screen(props: { clusterId: string }) {
  const value = fetchThing(props.clusterId);
  return <div>{value}</div>;
}
export function RouteAccessor() {
  const params = useParams<{ clusterId?: string }>();
  return (
    <Show when={params.clusterId}>{(id) => <Screen clusterId={id()} />}</Show>
  );
}

// Withheld: the imported `save` is a project `function`, not an action, so
// `SC9012` stays.
export function Save() {
  const run = useAction(save as never);
  return <button onClick={() => run()}>save</button>;
}
