import { createTrackedEffect, until } from "solid-js";
export function App() {
  const run = async () => { void until(() => 1); };
  createTrackedEffect(() => { run().catch(() => {}); });
  return <div></div>;
}
