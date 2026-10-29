// ADR 0511 distinction: the literal's slot is a getter that throws, so destructuring it ends the body before the effect;
// nothing reads the pending memo. Only plain data properties are followed.
import { createContext, createMemo, createRenderEffect, useContext } from "solid-js";
import type { Accessor } from "solid-js";

const ValCtx = createContext<{ a1: Accessor<number> }>();

function C1() {
  const ctx = useContext(ValCtx);
  const { read } = {
    get read(): Accessor<number> {
      throw new Error("getter");
    },
  };
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    read();
    ctx.a1();
  });
  return <div></div>;
}

export function P1() {
  const a1 = createMemo(async () => 1);
  return (
    <div>
      <ValCtx value={{ a1 }}>
        <C1 />
      </ValCtx>
    </div>
  );
}
