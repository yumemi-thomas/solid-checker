// app-game abr-viewer routes.tsx:9: a lazy route imports the page module and picks
// the component off it.
import { lazy } from "solid-js";

export const routes = [
  {
    path: "/",
    component: lazy(() => import("./escaped-lazy-page").then((module) => ({ default: module.Page }))),
  },
];
