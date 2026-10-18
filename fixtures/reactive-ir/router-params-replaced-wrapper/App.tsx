import { Show } from "solid-js";
import { useParams } from "@solidjs/router";
import type { RouterUtils } from "@solidjs/router";

// ADR 0324 refusal: the project sets `paramsWrapper`, which replaces the
// object `useParams()` returns (`index.js:1306`). The premise is not applied
// anywhere in the project, so the settled site of `../App.tsx` stays open
// here.
export const utils: Partial<RouterUtils> = {
  paramsWrapper: (getParams) => getParams(),
};

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
