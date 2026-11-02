import { createSignal } from "solid-js";

function h1(cb: () => unknown) {
  cb();
  return 1;
}

export function C1() {
  const [r1, setR1] = createSignal(3);
  const dir1 = () => {
    h1(() => {
      r1();
    });
    return () => {};
  };
  return (
    <div>
      <p ref={dir1()} data-view="v1">x</p>
    </div>
  );
}
