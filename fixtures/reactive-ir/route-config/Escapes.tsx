import { createSignal } from "solid-js";
import { createRouter } from "@solidjs/router";
import * as Routing from "@solidjs/router";

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

// Uncertifiable: a namespace member is not an exact import.
function Spaced() {
  const value = count();
  return <p>{value}</p>;
}

export const SpacedRouter = Routing.createRouter({ routes: [{ path: "/spaced", component: Spaced }] });

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
