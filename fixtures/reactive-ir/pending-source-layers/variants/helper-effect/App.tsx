import { createEffect, createMemo } from "solid-js";
import type { Accessor } from "solid-js";

function runNow(callback: () => unknown) {
  callback();
  return 1;
}

function HelperChild(props: { data: Accessor<number> }) {
  createEffect(
    () => 1,
    () => {
      runNow(() => {
        props.data();
      });
    },
  );
  return <p>static</p>;
}

// Violation (SC5001, in `HelperChild`): a user effect's apply, run in the
// first render's tail flush, reads the prop through the helper's callback.
export function PropHelper() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <HelperChild data={data} />
    </div>
  );
}
