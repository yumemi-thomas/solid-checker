/** @jsxImportSource @solidjs/web */
import { createTimeoutLoop } from "@solid-primitives/timer";
createTimeoutLoop(() => {}, () => false);
export default function App() { return <p>ready</p>; }
