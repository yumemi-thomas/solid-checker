import { createMemo } from "solid-js";
import type { Accessor } from "solid-js";

// Exported, so a caller outside the analyzed project may call it from
// anywhere: the walk does not claim it.
export function memoFromOtherFile(source: Accessor<number>): void {
  createMemo(() => source() + 1);
}
