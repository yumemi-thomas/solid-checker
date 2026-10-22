import { createSignal } from "solid-js";
import { createRouter } from "@solidjs/router";

const [count] = createSignal(0);

// Positive: route definitions written in `createRouter`'s `routes`, and in a
// definition's `children`. The audited router renders each `component` with
// `createComponent` and never calls it, so each body read is a violation.
function Home() {
  const value = count();
  return <p>{value}</p>;
}

function Layout(props: { children?: unknown }) {
  const value = count();
  return <div>{value}{props.children}</div>;
}

function Nested() {
  const value = count();
  return <p>{value}</p>;
}

export const Router = createRouter({
  routes: [
    { path: "/", component: Home },
    { path: "/area", component: Layout, children: [{ path: "/inner", component: Nested }] },
  ],
});
