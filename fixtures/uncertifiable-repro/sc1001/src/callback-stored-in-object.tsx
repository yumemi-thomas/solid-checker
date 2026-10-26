// kui feature-topics TopicsRoute.tsx:824 (useTabQuery, :148): a callback reading props is handed to a project function that puts it in
// an object (`{ key, load }`) it hands to `useQuery`, which binds it through a registry's interface method (kernel useQuery.ts:257).
import { createMemo } from "solid-js";

interface Registry {
  readonly internals: { bind(key: string, load: () => string): void };
}

const shared: Registry = { internals: { bind: () => {} } };

function useQuery(options: { key: () => string; load: () => string; registry?: Registry }) {
  const registry = options.registry ?? shared;
  return createMemo(() => {
    const key = options.key();
    registry.internals.bind(key, options.load);
    return key;
  });
}

function useTabQuery(key: () => string, load: () => string) {
  const query = useQuery({ key, load });
  return { state: query };
}

export function Consumers(props: { clusterId: string }) {
  const consumers = useTabQuery(
    () => "consumers",
    () => `topic:${props.clusterId}`,
  );
  return <p>{consumers.state()}</p>;
}
