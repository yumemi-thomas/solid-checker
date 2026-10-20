import { createSignal } from "solid-js";
import { createRouter } from "@solidjs/router";

const [count] = createSignal(0);

// Uncertifiable: definitions held in a constant, not written at `routes`.
function Held() {
  const value = count();
  return <p>{value}</p>;
}

const held = [{ path: "/held", component: Held }];
export const HeldRouter = createRouter({ routes: held });

// Uncertifiable: a definition built by a project call, which may call it.
function Wrapped() {
  const value = count();
  return <p>{value}</p>;
}

function wrap<T>(definition: T): T {
  return definition;
}

// Uncertifiable: a `component` key of a nested object that is no definition.
function Described() {
  const value = count();
  return <p>{value}</p>;
}

export const OtherRouter = createRouter({
  routes: [wrap({ path: "/wrapped", component: Wrapped }), { path: "/info", info: { component: Described } }],
});

// Uncertifiable: an element that is not itself a definition literal, a
// `component` inside a computed key, and one a spread can overwrite.
function Picked() {
  const value = count();
  return <p>{value}</p>;
}

function Keyed() {
  const value = count();
  return <p>{value}</p>;
}

function Overwritten() {
  const value = count();
  return <p>{value}</p>;
}

const override: Record<string, unknown> = {};

export const ShapedRouter = createRouter({
  routes: [
    { component: Picked, route: { path: "/picked" } }.route,
    { path: "/keyed", [JSON.stringify({ component: Keyed })]: true },
    { path: "/over", component: Overwritten, ...override },
  ],
});

// Uncertifiable: the router calls `route.preload(...)` on the route record,
// so a method, or a named function that may bind `this`, receives a record
// holding `component`.
function Bound() {
  const value = count();
  return <p>{value}</p>;
}

function Named() {
  const value = count();
  return <p>{value}</p>;
}

function loadNamed() {
  return undefined;
}

export const BoundRouter = createRouter({
  routes: [
    { path: "/bound", component: Bound, preload() { return undefined; } },
    { path: "/named", component: Named, preload: loadNamed },
  ],
});

// Uncertifiable: the router calls `config.transformUrl(...)` on the config,
// so a method there could reach `this.routes`.
function Transformed() {
  const value = count();
  return <p>{value}</p>;
}

export const TransformRouter = createRouter({
  routes: [{ path: "/transformed", component: Transformed }],
  transformUrl(url: string) {
    return url;
  },
});
