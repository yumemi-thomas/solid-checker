import { createMemo } from "solid-js";

export const remote = createMemo(async () => 1);
// Uncertifiable (in `App.tsx`): the module's scope runs code besides creating.
JSON.stringify(1);
