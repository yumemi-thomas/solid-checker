import { createContext, createMemo, createRenderEffect, useContext } from "solid-js";
import type { Accessor } from "solid-js";

const DataCtx = createContext<{ data: Accessor<number> }>();

function GetterChild() {
  const ctx = useContext(DataCtx);
  createRenderEffect(
    () => 1,
    () => {
      ctx.data();
    },
  );
  return <p>static</p>;
}

// Uncertifiable (in `GetterChild`): the provided value is a getter, which
// runs code when read; ADR 0510 admits a provider whose value object holds
// only names the parent created.
export function GetterParent() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <DataCtx value={{ get data() { return data; } }}>
        <GetterChild />
      </DataCtx>
    </div>
  );
}
