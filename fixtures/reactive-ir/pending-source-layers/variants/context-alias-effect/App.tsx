import { createContext, createEffect, createMemo, useContext } from "solid-js";
import type { Accessor } from "solid-js";

const DataCtx = createContext<{ shared: Accessor<number> }>();

function AliasChild() {
  const ctx = useContext(DataCtx);
  const read = ctx.shared;
  createEffect(
    () => 1,
    () => {
      read();
    },
  );
  return <p>static</p>;
}

// Violation (SC5001, in `AliasChild`): a user effect reads a `const` alias of
// the provided memo in the first render's tail flush.
export function ContextAlias() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <DataCtx value={{ shared: data }}>
        <AliasChild />
      </DataCtx>
    </div>
  );
}
