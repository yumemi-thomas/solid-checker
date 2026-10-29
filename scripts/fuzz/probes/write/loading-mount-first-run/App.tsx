// ADR 0511: a child <Loading> renders during the parent's tree; its render effect's first apply runs on the call, inside the
// boundary's children computed, so a write there throws REACTIVE_WRITE_IN_OWNED_SCOPE as under <Show when={true}>.
import { Loading, createRenderEffect, createSignal } from "solid-js";

function C1() {
  const [s1, setS1] = createSignal(0);
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    setS1(2);
  });
  return <div></div>;
}

export function P1() {
  return (
    <div>
      <Loading fallback={0}>
        <C1 />
      </Loading>
    </div>
  );
}
