// app-game night-defense routes.tsx:8: the route table of exported-lazy-route.tsx.
import { lazy } from "solid-js";

export const routes = [{ path: "/", component: lazy(() => import("./exported-lazy-route")) }];
