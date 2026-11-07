import { createEffect, createMemo, createSignal, createStore } from "solid-js";

export function App() {
  const [s1, setS1] = createStore({ v: 0 });
  const f1 = async () => {
    await Promise.resolve();;
    s1.v = 7;
    return 1;
  };
  const m1 = createMemo(async () => {
    void 0;
    await f1();;
    return 1;
  });
  createEffect(() => {
    return m1();
  }, (v) => {});
  const m2 = createMemo(async () => {
    await f1();;
    return 1;
  });
  createEffect(() => {
    return m2();
  }, (v) => {});
  return (
    <div>
    </div>
  );
}
