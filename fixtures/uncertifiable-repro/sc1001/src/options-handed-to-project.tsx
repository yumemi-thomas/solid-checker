// kui ConsumersRoute.tsx:89 (kernel data/query/useQuery.ts:257): a callback reading props sits in an options object handed to a project
// function, which hands it on to `registry.internals.bind(key, options.load)`: a method the registry's interface declares, so the call
// does not name the function that runs.
import { createMemo } from "solid-js";

interface Registry {
  readonly internals: { bind(key: string, load: () => Promise<string>): void };
}

function createRegistry(): Registry {
  const loaders = new Map<string, () => Promise<string>>();
  return { internals: { bind: (key, load) => void loaders.set(key, load) } };
}

const shared = createRegistry();

function useQuery(options: { key: () => string; load: () => Promise<string>; registry?: Registry }) {
  const registry = options.registry ?? shared;
  return createMemo(() => {
    const key = options.key();
    registry.internals.bind(key, options.load);
    return key;
  });
}

export function Groups(props: { clusterId: string }) {
  const groups = useQuery({
    key: () => `groups:${props.clusterId}`,
    load: () => Promise.resolve(props.clusterId),
  });
  return <p>{groups()}</p>;
}
