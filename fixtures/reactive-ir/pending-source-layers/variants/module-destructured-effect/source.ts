import { createMemo } from "solid-js";

// ADR 0510: a module whose scope only creates nodes.
export const remote = createMemo(async () => 1);
