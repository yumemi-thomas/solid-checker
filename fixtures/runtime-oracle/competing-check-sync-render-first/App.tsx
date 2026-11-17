import { Loading, createEffect, createSignal, onSettled } from "solid-js";

export function C1() {
  const u1 = () => {
    createEffect(async () => 1, () => {}, { sync: true });
  };
  const hc1 = () => {
    u1();
  };
  const hc2 = () => {
    hc1();
  };
  const hc3 = () => {
    hc2();
  };
  const hc4 = () => {
    hc3();
  };
  const hc5 = () => {
    hc4();
  };
  onSettled(() => {
    hc5();
  });
  return (
    <div>
      <p data-view="v1"><Loading fallback={0}><i>{(u1(), "x")}</i></Loading></p>
    </div>
  );
}
