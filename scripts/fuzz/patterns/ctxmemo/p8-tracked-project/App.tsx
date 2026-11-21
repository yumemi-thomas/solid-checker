import { createContext, createMemo, createSignal, createTrackedEffect } from "solid-js";
const Ctx = createContext<number>();
export function Host() {
  const [n] = createSignal(0);
  createTrackedEffect(() => {
    n();
    const made = (
      <Ctx value={1}>
        {createMemo(() => 1)()}
      </Ctx>
    );
    void made;
  });
  return <p />;
}
