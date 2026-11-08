import { createSignal } from "solid-js";
const [c] = createSignal(1);
function h() { return c(); }
export function App() { [1].map(h); return <i/>; }
