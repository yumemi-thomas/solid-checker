import { createMemo } from "solid-js";

export const remote = createMemo(async () => 1);
// Uncertifiable (in `App.tsx`): a top-level await lets microtasks run, and
// the memo settle, before the modules that import this one evaluate.
await 0;
