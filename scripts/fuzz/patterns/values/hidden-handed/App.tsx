import { createSignal } from "solid-js";
const [c] = createSignal(1);
function h() { return c(); }
const o = { toString() { return String([1].map(h)); } };
export function App() { void `${o}`; return <i/>; }
