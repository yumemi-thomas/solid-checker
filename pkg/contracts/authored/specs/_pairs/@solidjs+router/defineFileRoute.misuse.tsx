import { defineFileRoute } from "@solidjs/router/fs";
import { createStore } from "solid-js";

// The identity helper preserves the caller's reactive store. Only the
// component-body read after the helper call is a strict-read misuse.
export default function App() {
  const [config] = createStore({ info: { label: "route" } });
  const route = defineFileRoute("/stop", config);
  const label = route.info!.label;
  return <p>{label}</p>;
}
