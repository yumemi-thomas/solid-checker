import { createTrackedEffect, resolve } from "solid-js";
export function App() {
  const inner = () => { void resolve(() => 3); };
  const run = async () => { inner(); };
  createTrackedEffect(() => { run().catch(() => {}); });
  return <div></div>;
}
