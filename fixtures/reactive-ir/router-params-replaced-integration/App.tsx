import { Show } from "solid-js";
import { useParams } from "@solidjs/router";
import type { RouterIntegration } from "@solidjs/router";

// ADR 0324 refusal: the project names a `RouterIntegration`, the type a custom
// integration implements to supply `utils.paramsWrapper`. The premise is not
// applied anywhere in the project.
export declare const integration: RouterIntegration | undefined;

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
