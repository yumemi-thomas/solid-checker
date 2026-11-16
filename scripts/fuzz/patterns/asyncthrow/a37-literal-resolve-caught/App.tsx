import { createTrackedEffect, resolve } from "solid-js";
export function App() {
  createTrackedEffect(() => { (async () => { void resolve(() => 1); })().catch(() => {}); });
  return <div></div>;
}
