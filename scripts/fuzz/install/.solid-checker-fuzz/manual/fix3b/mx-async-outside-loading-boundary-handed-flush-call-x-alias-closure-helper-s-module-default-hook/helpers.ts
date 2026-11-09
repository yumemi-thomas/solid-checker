import { createMemo } from "solid-js";

export default function useVals() {
  const a1 = createMemo(async () => 1);
  return { a1 };
}
