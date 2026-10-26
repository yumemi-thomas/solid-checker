// kui feature-topics TopicsRoute.tsx:458 (useClusterReadOnly, :181): a callback reading props is
// handed to a project function that keeps it (a project `createQuery`).
import { createSignal } from "solid-js";

function createQuery<T>(key: () => string[], fetcher: (key: string[]) => Promise<T>) {
  const [data, setData] = createSignal<T>();
  return { data, refetch: () => fetcher(key()).then((value) => setData(() => value)) };
}

export function SkillProposalCard(props: { messageId: string; proposal: string }) {
  const query = createQuery(() => (props.proposal ? ["skills", "proposal", props.messageId, props.proposal] : ["skills"]), async (key) => key.join("/"));
  return <button onClick={() => void query.refetch()}>{query.data() ?? "Load"}</button>;
}
