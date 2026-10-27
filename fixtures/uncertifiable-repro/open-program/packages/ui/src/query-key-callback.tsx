// kui feature-clusters/src/BrokersScreen.tsx:72 (and 16 more `key` callbacks):
// an exported component hands a callback reading a prop to a project function
// (`useQuery`) that hands it to a Solid primitive. Closed, the read is proven to
// run in the memo's tracked compute; open, the component's callers are unknown.
import { createMemo, untrack } from "solid-js";

function useQuery(options: { key: () => string }) {
  const bound = createMemo(() => options.key(), { lazy: true });
  return { state: () => bound(), reload: () => untrack(options.key) };
}

export function BrokersScreen(props: { id: string }) {
  const brokers = useQuery({ key: () => `clusters/${props.id}/brokers` });
  return <p>{brokers.state()}</p>;
}

export function Page() {
  return <BrokersScreen id="dev" />;
}
