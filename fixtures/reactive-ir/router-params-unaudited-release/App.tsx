import { Show } from "solid-js";
import { useParams } from "@solidjs/router";

// ADR 0324 refusal: this project resolves `@solidjs/router` 2.0.0-next.18,
// which no one read. The premise is not applied and the project carries an
// SC9014 notice naming the router and the audited release.
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
