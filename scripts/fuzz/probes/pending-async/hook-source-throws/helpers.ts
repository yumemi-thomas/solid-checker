import { createMemo } from "solid-js";

export function useVals() {
  const a1 = createMemo(async () => 1);
  JSON.parse("{");
  return { a1 };
}
