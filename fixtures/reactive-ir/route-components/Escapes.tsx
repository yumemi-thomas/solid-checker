import { createSignal } from "solid-js";
import { createRoute } from "@tanstack/solid-router";
import * as Router from "@tanstack/solid-router";
import { root } from "./Routes";

const [count] = createSignal(0);

// Uncertifiable: the options are not a literal at the call, so the value is
// stored in an object the premise does not follow.
function Held() {
  const value = count();
  return <p>{value}</p>;
}

const options = { getParentRoute: () => root, path: "/held", component: Held };
export const held = createRoute(options);

// Uncertifiable: a namespace member is not an exact factory import.
function Spaced() {
  const value = count();
  return <p>{value}</p>;
}

export const spaced = Router.createRoute({
  getParentRoute: () => root,
  path: "/spaced",
  component: Spaced,
});

// Uncertifiable: a key the audit did not cover.
function Loaded() {
  const value = count();
  return <p>{value}</p>;
}

export const loaded = createRoute({
  getParentRoute: () => root,
  path: "/loaded",
  loader: Loaded,
});

// Uncertifiable: a local `createRoute` shadows the import.
function Shadowed() {
  const value = count();
  return <p>{value}</p>;
}

export function local() {
  const createRoute = (route: object) => [route];
  return createRoute({ component: Shadowed });
}
