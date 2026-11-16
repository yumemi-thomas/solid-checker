import { createTrackedEffect, resolve } from "solid-js";
export function App() {
  const load = async () => {};
  const run = () => { void resolve(() => 2); void load(); };
  createTrackedEffect(() => { run(); });
  return <div></div>;
}
