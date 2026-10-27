// app-game packages/ogl-examples/draw-modes/draw-modes.tsx:91 with routes.tsx:43: a default export loaded by lazy() calls a package primitive that registers onCleanup, which a mounted component also calls.
import createRAF from "@solid-primitives/raf";
import { render } from "@solidjs/web";
import { lazy } from "solid-js";

function Ticker() {
  const [, start] = createRAF(() => console.debug("tick"));
  start();
  return <span />;
}

render(() => <Ticker />, document.body);

export const routes = [{ path: "/scene", component: lazy(() => import("./pkg-raf")) }];

export default function Scene() {
  const [, start] = createRAF(() => console.debug("frame"));
  start();
  return document.createElement("canvas");
}
