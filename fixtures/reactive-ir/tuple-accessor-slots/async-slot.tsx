import { createMemo, type Accessor } from "solid-js";
function useNow() {
  const now = createMemo(async () => { await 0; return 1; });
  return [now] as const;
}
export function AsyncSlot() {
  const [now] = useNow();
  return <time>{now()}</time>;
}
