import { Loading, createEffect, createMemo } from "solid-js";
import type { Accessor } from "solid-js";

function LoadingChild(props: { data: Accessor<number> }) {
  createEffect(
    () => 1,
    () => {
      props.data();
    },
  );
  return <p>static</p>;
}

// Violation (SC5001, in `LoadingChild`): nothing below the boundary reads a
// pending value, so it shows its content and the first render's tail flush
// runs the user effect while the memo is pending.
export function LoadingHolder() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <Loading fallback={0}>
        <LoadingChild data={data} />
      </Loading>
    </div>
  );
}
