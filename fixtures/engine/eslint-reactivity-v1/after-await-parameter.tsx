import { createMemo, type Accessor } from "solid-js";

function install(data: Accessor<number>) {
  return createMemo(async () => {
    const ready = await Promise.resolve(true);
    return ready ? data() : 0;
  });
}

export { install };
