import { Loading, createEffect, createSignal } from "solid-js";

export function C1() {
  createEffect(() => {
    return 1;
  }, (v) => {
    Loading({ fallback: 0, children: 1 });
  });
  return (
    <div>
    </div>
  );
}
