import { createMemo, type Accessor } from "solid-js";

function install(props: { data: Accessor<number> }) {
  return createMemo(async () => {
    await Promise.resolve();
    return props.data();
  });
}

export { install };
