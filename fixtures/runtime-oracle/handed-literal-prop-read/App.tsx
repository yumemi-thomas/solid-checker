import { createMemo, createSignal } from "solid-js";

type C1Props = { m1: number };

function h1(cb: () => unknown) {
  cb();
  return 1;
}

function C1(props: C1Props) {
  const dir1 = () => {
    h1(() => {
      (props as typeof props).m1;
    });
    return () => {};
  };
  return (
    <div>
      <p ref={dir1()} data-view="v1">x</p>
    </div>
  );
}

export function P1() {
  const m1 = createMemo(() => 1);
  return (
    <div>
      <C1 m1={m1()} />
    </div>
  );
}
