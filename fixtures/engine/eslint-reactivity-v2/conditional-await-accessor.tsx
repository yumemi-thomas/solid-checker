import { createMemo, type Accessor } from "solid-js";

function install(data: Accessor<number>, shouldWait: boolean) {
  return createMemo(async () => {
    if (shouldWait) await Promise.resolve();
    return data();
  });
}

export { install };
