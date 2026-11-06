const log = (...a: any[]) => (window as any).__log(...a);
import { use } from "./lib";
export function App() { const s = use(); const r = s.v; log(r); return <p>{r}</p>; }
