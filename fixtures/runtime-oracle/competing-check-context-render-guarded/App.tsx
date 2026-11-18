import { createContext, createRenderEffect, createSignal, useContext } from "solid-js";

const Ctx = createContext<number>();

export function C1() {
  const [t1, setT1] = createSignal(1);
  createRenderEffect(() => {
    return t1();
  }, (v) => {
    if (v === 1) return;
    {
      useContext(Ctx);
    }
  });
  return (
    <div>
      <button data-act="b1" onClick={() => {
        setT1(2);
      }}>go</button>
    </div>
  );
}
