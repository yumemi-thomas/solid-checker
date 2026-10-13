// Imports the re-exported name from the project module, not from solid-js:
// the re-export in `boundary.ts` is what reaches the gap.
import { createRevealOrder } from "./boundary";

export const order = createRevealOrder;
