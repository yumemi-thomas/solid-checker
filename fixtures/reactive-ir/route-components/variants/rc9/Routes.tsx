import { createSignal } from "solid-js";
import {
  createFileRoute,
  createRootRoute,
  createRootRouteWithContext,
  createRoute,
  createRouter,
} from "@tanstack/solid-router";

const [count] = createSignal(0);

// Positive: each of the four keys of a file route's options. The audited
// router renders these values (`<Dynamic component={…}/>`, `<X/>`) and never
// calls one, so each body is a component body and its read is a violation.
function Home() {
  const value = count();
  return <p>{value}</p>;
}

function Pending() {
  const value = count();
  return <p>{value}</p>;
}

function Failed() {
  const value = count();
  return <p>{value}</p>;
}

function Missing() {
  const value = count();
  return <p>{value}</p>;
}

export const Route = createFileRoute("/")({
  component: Home,
  pendingComponent: Pending,
  errorComponent: Failed,
  notFoundComponent: Missing,
});

// Positive: a root route built through the direct factory (the rc.8 corpus
// app's shape) and through the curried context factory, and a code route.
function Root() {
  const value = count();
  return <div>{value}</div>;
}

export const root = createRootRoute({ component: Root });

function Shell() {
  const value = count();
  return <div>{value}</div>;
}

export const contextual = createRootRouteWithContext<{ user: string }>()({ component: Shell });

function About() {
  const value = count();
  return <p>{value}</p>;
}

export const about = createRoute({
  getParentRoute: () => root,
  path: "/about",
  component: About,
});

// Positive: an inline component value, and a router default.
export const inline = createRoute({
  getParentRoute: () => root,
  path: "/inline",
  component: () => {
    const value = count();
    return <p>{value}</p>;
  },
});

function Lost() {
  const value = count();
  return <p>{value}</p>;
}

export const router = createRouter({ routeTree: root, defaultNotFoundComponent: Lost });
