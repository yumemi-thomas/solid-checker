import { onCleanup } from "solid-js";
import "./cycle-b";
export function enterCycle() { cyclic.cycleTicker(); }
export const cyclic = { cycleTicker() { onCleanup(() => {}); } };
