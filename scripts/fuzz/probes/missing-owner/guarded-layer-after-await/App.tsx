// ADR 0493 review negative: a getOwner() guard around a helper layer after an await skips it with no owner; the checker must not prove NO_OWNER_EFFECT.
import { createEffect, getOwner } from "solid-js";

function run(cb: () => void) {
  cb();
}

export function C1() {
  const load = async () => {
    await Promise.resolve();
    if (getOwner()) {
      run(() => {
        createEffect(() => 1, () => {});
      });
    }
  };
  void load();
  return <div></div>;
}
