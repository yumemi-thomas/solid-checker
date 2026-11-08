import { createSignal } from "solid-js";
const [c] = createSignal(1);
function h() { return c(); }
const o = { toString() { return String(h.apply(null)); } };
export function App() { void `${o}`; return <i/>; }
