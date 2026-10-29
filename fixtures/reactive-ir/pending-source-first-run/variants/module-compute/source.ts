import { createMemo } from "solid-js";

// Uncertifiable (in `App.tsx`): the compute runs code, not one literal.
export const remote = createMemo(async () => JSON.parse("1") as number);
