// kui feature-clusters/src/ClustersRoute.tsx:359 (and 42 more calls of its kind):
// an exported component passes a prop typed `string` to a project function that
// wraps it in a literal handed to a method of its `api` parameter, inside a
// callback a project function keeps. Closed, the callers of the component prove
// the prop a primitive (ADR 0213); open, they are not enumerable.
import { createContext, useContext } from "solid-js";

interface Api {
  get(path: string, init: { params: { path: { id: string } } }): Promise<{ ok: boolean }>;
}
const KuiContext = createContext<{ api: Api }>();
const useKui = () => useContext(KuiContext)!;

async function fetchBrokers(api: Api, id: string) {
  return api.get("/clusters/{id}/brokers", { params: { path: { id } } });
}

function createMutation(run: () => Promise<{ ok: boolean }>) {
  return { run: () => run() };
}

export function BrokersScreen(props: { id: string }) {
  const kui = useKui();
  const remove = createMutation(() => fetchBrokers(kui.api, props.id));
  return <button onClick={() => void remove.run()}>go</button>;
}

export function Page() {
  return <BrokersScreen id="dev" />;
}
