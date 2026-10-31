import { createMemo, createRenderEffect, createRoot } from "solid-js";

export const remote = createMemo(async () => 1);
createRoot(() => {
  createRenderEffect(() => remote(), () => {});
});
await new Promise((done) => setTimeout(done, 50));
