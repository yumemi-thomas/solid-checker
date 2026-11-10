import { createSignal, onSettled } from "solid-js";

export function C1() {
  const f1 = async () => {
    await Promise.resolve();
    onSettled.bind(null)(() => () => {});
  };
  f1();
  return (
    <div>
    </div>
  );
}
