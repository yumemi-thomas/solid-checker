// ADR 0495 review negative: a render effect run under a saved owner in the helper call's own argument lowers the effect-callback marker, so the later ownerless memo does not throw; the checker must not prove PRIMITIVE_IN_EFFECT_CALLBACK.
import { createEffect, createMemo, createRenderEffect, createSignal, getOwner, runWithOwner } from "solid-js";

function run(_n: number, cb: () => void) {
  cb();
}

export function C1() {
  const [count] = createSignal(0);
  const owner = getOwner();
  const lower = () => {
    runWithOwner(owner, () => createRenderEffect(() => 0, () => {}));
    return 0;
  };
  createEffect(() => count(), () => {
    run(lower(), () => {
      createMemo(() => 1);
    });
  });
  return <div></div>;
}
