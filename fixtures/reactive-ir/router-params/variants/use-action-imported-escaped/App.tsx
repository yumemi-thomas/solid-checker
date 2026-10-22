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

// Withheld: `register.ts` hands `save` to `Object.assign`, which can give it
// an own `apply`, so `SC9012` stays.
export function Save() {
  const run = useAction(save);
  return <button onClick={() => run(new FormData())}>save</button>;
}
