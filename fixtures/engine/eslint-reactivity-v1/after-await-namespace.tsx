import * as solid from "solid-js";

const [data] = solid.createSignal(1);
export const value = solid.createMemo(async () => {
  await Promise.resolve();
  return data();
});
