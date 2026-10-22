import { Show } from "solid-js";
import { useAction, useParams, useSubmissions } from "@solidjs/router";
import { save } from "./actions";
import * as actions from "./actions";

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

// Withheld: the action's module is also reached as a namespace, whose
// object no reference census bounds, so `SC9012` stays.
export function Save() {
  const run = useAction(save);
  const pending = useSubmissions(actions.save);
  return <button onClick={() => run(new FormData())}>save {pending.length}</button>;
}
