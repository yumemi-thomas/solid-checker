// ADR 0511: a user effect under <Loading> whose tree reads the body's own async memo is held to the reveal and warns
// STRICT_READ_UNTRACKED; the checker keeps both open outcomes (SC1001 and SC5001 uncertifiable), never one alone.
import { Loading, createEffect, createMemo } from "solid-js";

function C1() {
  const a1 = createMemo(async () => 1);
  createEffect(() => {
    return 1;
  }, (v) => {
    a1();
  });
  return <div>{a1()}</div>;
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
