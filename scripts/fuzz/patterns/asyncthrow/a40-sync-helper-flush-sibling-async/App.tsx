import { createTrackedEffect, flush } from "solid-js";
export function App() {
  const load = async () => {};
  const run = () => { flush(); void load(); };
  createTrackedEffect(() => { run(); });
  return <div></div>;
}
