// kui feature-topics TopicsRoute.tsx:530-536: an async callback handed to a project function reads
// a prop in both branches of a conditional that awaits; the classifier reads the second branch
// as after the first `await`.
function createMutation<A, R>(run: (arg: A) => Promise<R>) {
  return (arg: A) => run(arg);
}

export function Topics(props: { clusterId: string }) {
  const bulk = createMutation(async (kind: "purge" | "delete") =>
    kind === "delete" ? await fetch(`/c/${props.clusterId}/delete`) : await fetch(`/c/${props.clusterId}/purge`),
  );
  return <button onClick={() => void bulk("purge")}>Purge</button>;
}
