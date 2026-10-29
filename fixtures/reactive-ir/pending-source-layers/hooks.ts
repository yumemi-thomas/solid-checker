import { createMemo } from "solid-js";

// ADR 0511: an inert hook creates the memo and returns it in a fresh object.
export function useData() {
  const data = createMemo(async () => 1);
  return { data };
}

// Not inert: it runs code after the creation (and throws there).
export function useDataThenParse() {
  const data = createMemo(async () => 1);
  JSON.parse("{");
  return { data };
}
