import { createEffect, createMemo, createSignal } from "solid-js";

export function App() {
  const [r1, setR1] = createSignal(3);
  const f1 = async () => {
    void 0;
    await Promise.resolve();;
    r1.bind(null)();
    return 1;
  };
  const m1 = createMemo(async () => {
    await Promise.resolve();;
    await f1();;
    return 1;
  });
  createEffect(() => {
    return m1();
  }, (v) => {});
  return (
    <div>
    </div>
  );
}
