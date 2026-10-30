import { createMemo } from "solid-js";

export function useData() {
  const data = createMemo(async () => 1);
  return { data };
}
