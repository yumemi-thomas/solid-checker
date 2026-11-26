import { createOptimistic, onSettled } from "solid-js";

// ADR 1224: `onSettled` under the child's owner is a tracked effect whose first run comes in `render`'s tail flush, before
// the async compute's first answer: rc.21 throws OPTIMISTIC_WRITE_BEFORE_LOAD at the setter. A violation.
function Kid(props: { run: () => void }) {
  onSettled(() => {
    props.run();
  });
  return <p />;
}

export function App() {
  const [, setO] = createOptimistic(async () => 1);
  return (
    <div>
      <Kid run={() => setO(6)} />
    </div>
  );
}
