import { createTrackedEffect, flush } from "solid-js";
export function App() {
  const run = async () => { flush(); };
  createTrackedEffect(() => { run().catch(() => {}); });
  return <div></div>;
}
