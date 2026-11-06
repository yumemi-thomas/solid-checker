import { createMemo, Loading } from "solid-js";
function useX() {
  const a = createMemo(async () => { await 0; return 1; });
  return [a] as const;
}
export function Covered() {
  const [m] = useX();
  return <Loading fallback="w"><time>{m()}</time></Loading>;
}
