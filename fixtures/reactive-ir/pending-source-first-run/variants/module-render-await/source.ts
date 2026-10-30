import { createMemo } from "solid-js";

export const remote = createMemo(async () => 1);
// Uncertifiable (in `App.tsx`): a top-level await lets microtasks run, and
// the memo may settle before the render effect's compute first reads it; a
// settled read needs no boundary (ADR 0513).
await 0;
