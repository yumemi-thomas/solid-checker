import { createEffect, createMemo, flush, resolve } from "solid-js";

// `flush()` in the memo drains the apply under the memo's observer.
export function Drained() {
  createEffect(() => 1, () => {
    resolve(() => 1); // SC2004 uncertifiable
  });
  const m = createMemo(() => {
    flush();
    return 1;
  });
  return <div>{m()}</div>;
}
