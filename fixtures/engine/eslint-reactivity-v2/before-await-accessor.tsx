import { createMemo, type Accessor } from "solid-js";

function install(data: Accessor<number>) {
  return createMemo(async () => {
    const current = data();
    await Promise.resolve();
    return current;
  });
}

export { install };
