import { createEffect, createOptimistic } from "solid-js";

// ADR 1224: the child's apply calls the prop only on a later run (`v > 1` is false on the first), after the answer:
// Chrome is silent. Uncertifiable.
function Kid(props: { run: () => void }) {
  createEffect(
    () => 1,
    (v) => {
      if (v > 1) props.run();
    },
  );
  return <p />;
}

export function App() {
  const [, setO] = createOptimistic(async () => 1);
  return (
    <div>
      <Kid run={() => setO(8)} />
    </div>
  );
}
