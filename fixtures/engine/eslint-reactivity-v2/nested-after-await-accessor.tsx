import { createMemo, type Accessor } from "solid-js";

function install(data: Accessor<number>) {
  return createMemo(async () => {
    await Promise.resolve();
    return () => data();
  });
}

export { install };
