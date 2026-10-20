import { onCleanup } from "solid-js";
export const local = { localTicker(fn: () => void) { onCleanup(fn); } };
