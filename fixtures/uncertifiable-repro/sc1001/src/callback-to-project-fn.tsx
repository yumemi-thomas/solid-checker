// kui feature-topics TopicsRoute.tsx:458 (useClusterReadOnly, :181): a callback reading props is wrapped by a project helper into
// the `key`/`load` options of another project function (`useQuery`), which reads `key` in a memo and binds `load` through a
// registry's interface method (kernel useQuery.ts:257).
import { createMemo, createSignal } from "solid-js";

interface Registry {
  readonly internals: { bind(key: string, load: () => Promise<boolean>): void };
}

const shared: Registry = { internals: { bind: () => {} } };

function useQuery(options: { key: () => string; load: () => Promise<boolean>; registry?: Registry }) {
  const registry = options.registry ?? shared;
  return createMemo(() => {
    const key = options.key();
    registry.internals.bind(key, options.load);
    return key;
  });
}

function useClusterReadOnly(clusterId: () => string) {
  const query = useQuery({ key: () => `cluster-write-state|${clusterId()}`, load: async () => clusterId().length > 0 });
  return () => query() !== "";
}

function TopicsScreen(props: { clusterId: string }) {
  const readOnly = useClusterReadOnly(() => props.clusterId);
  return <button disabled={readOnly()}>New topic</button>;
}

export function App() {
  const [cluster] = createSignal("main");
  return <TopicsScreen clusterId={cluster()} />;
}
