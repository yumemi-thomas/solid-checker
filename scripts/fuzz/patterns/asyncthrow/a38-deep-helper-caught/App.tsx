import { createTrackedEffect, resolve } from "solid-js";
export function App() {
  const inner = async () => { void resolve(() => 1); };
  const run = () => { inner().catch(() => {}); };
  createTrackedEffect(() => { run(); });
  return <div></div>;
}
