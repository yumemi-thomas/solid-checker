import { createMemo } from "solid-js";
const a = createMemo(async () => { await new Promise((r) => setTimeout(r, 30)); return 7; });
export default function B() { (window as any).__log?.("B", "body"); const v = a(); (window as any).__log?.("B", "v=" + v); return <i>{v}</i>; }
