import { Show } from "solid-js";
import { useAction, useParams } from "@solidjs/router";

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

// Withheld: a project wrapper hands `useAction` its parameter, which nothing
// proves is an arrow or the router's own action; a caller may pass a
// `function` that runs with the router context as `this`.
export function useMyAction(handler: Parameters<typeof useAction>[0]) {
  return useAction(handler);
}
