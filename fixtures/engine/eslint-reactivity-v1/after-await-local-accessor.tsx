import { createMemo } from "solid-js";

type Accessor<T> = () => T;
function install(data: Accessor<number>) {
  return createMemo(async () => {
    await Promise.resolve();
    return data();
  });
}

export { install };
