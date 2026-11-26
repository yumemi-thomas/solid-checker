import { createEffect, createOptimistic } from "solid-js";

// ADR 1224: the child's effect apply calls the callback prop in `render`'s synchronous tail flush, before the async
// compute's first answer: rc.21 throws OPTIMISTIC_WRITE_BEFORE_LOAD at the setter. A violation.
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
  const [, setO] = createOptimistic(async () => 1);
  return (
    <div>
      <Kid run={() => setO(2)} />
    </div>
  );
}
