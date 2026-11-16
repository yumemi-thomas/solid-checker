import { createTrackedEffect, flush } from "solid-js";
export function App() {
  const load = async () => {};
  const run = () => { void load(); flush(); };
  createTrackedEffect(() => { run(); });
  return <div></div>;
}
