import { createMemo } from "solid-js";

function install(data: () => number) {
  return createMemo(async () => {
    await Promise.resolve();
    return data();
  });
}

export { install };
