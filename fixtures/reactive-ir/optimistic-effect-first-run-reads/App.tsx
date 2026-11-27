import { createEffect, createOptimistic } from "solid-js";

// ADR 1224: the parent's tree reads the pending source, so the mount holds and the child's effect waits for the answer:
// no proof of a run before it. Uncertifiable.
function Kid(props: { run: () => void }) {
  createEffect(
    () => 1,
    () => {
      props.run();
    },
  );
  return <p />;
}

export function App() {
  const [o, setO] = createOptimistic(async () => 1);
  return (
    <div>
      <Kid run={() => setO(4)} />
      <p>{o()}</p>
    </div>
  );
}
