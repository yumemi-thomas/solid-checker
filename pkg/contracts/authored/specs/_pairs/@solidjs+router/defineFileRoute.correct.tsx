import { defineFileRoute } from "@solidjs/router/fs";
import { createStore } from "solid-js";

export default function App() {
  const [config] = createStore({ info: { label: "route" } });
  const route = defineFileRoute("/stop", config);
  return <p>{route.info!.label}</p>;
}
