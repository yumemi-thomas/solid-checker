// Misuse: constructing a hook memo inside a leaf owner.
import { onSettled } from "solid-js";
import { createRouter, memoryHistory, useHref } from "@solidjs/router";

function Reader() {
  onSettled(() => { useHref(() => "/stop/7"); });
  return <p>ready</p>;
}

const Router = createRouter({ routes: [{ path: "/stop/:id", component: () => <Reader /> }], history: memoryHistory("/stop/7") });
export default function App() { return <Router />; }
