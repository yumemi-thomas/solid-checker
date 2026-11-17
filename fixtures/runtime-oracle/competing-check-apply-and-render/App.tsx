import { Loading, createEffect, createSignal } from "solid-js";

export function C1() {
  const u1 = () => {
    createEffect(() => 1, () => {});
  };
  createEffect(() => {
    return 1;
  }, (v) => {
    u1();
  });
  return (
    <div>
      <p data-view="v1"><Loading fallback={0}><i>{(u1(), "x")}</i></Loading></p>
    </div>
  );
}
