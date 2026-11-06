import { createMemo, Loading } from "solid-js";
export function CaughtAtFive() {
  const a = createMemo(async () => 1);
  const f1 = () => a();
  const f2 = () => f1();
  const f3 = () => f2();
  const f4 = () => f3();
  const f5 = () => f4();
  const m = createMemo(() => { try { return f5(); } catch { return 0; } });
  return <Loading fallback={0}><p>{m()}</p></Loading>;
}
