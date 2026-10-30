import { createContext, createEffect, createMemo, useContext } from "solid-js";
import type { Accessor } from "solid-js";

const DataCtx = createContext<{ shared: Accessor<number> }>();

function ContextChild() {
  const ctx = useContext(DataCtx);
  createEffect(
    () => 1,
    () => {
      ctx.shared();
    },
  );
  return <p>static</p>;
}

// Violation (SC5001, in `ContextChild`): a user effect under the provider
// runs in the first render's tail flush, and meets the parent's memo pending.
export function ContextParent() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <DataCtx value={{ shared: data }}>
        <ContextChild />
      </DataCtx>
    </div>
  );
}
