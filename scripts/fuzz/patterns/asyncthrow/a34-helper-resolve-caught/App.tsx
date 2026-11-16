import { createTrackedEffect, resolve } from "solid-js";
export function App() {
  const run = async () => { void resolve(() => 1); };
  createTrackedEffect(() => { run().catch(() => {}); });
  return <div></div>;
}
