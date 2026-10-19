import { createMemo } from "solid-js";
export function keep(cb: () => void) {
  function run() {
    cb();
  }
  const m = createMemo(function () {
    cb();
    return 1;
  });
  void m;
  return run;
}
