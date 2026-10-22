import { createSignal } from "solid-js";
import { createRouter } from "@solidjs/router";

const [count] = createSignal(0);

// Withheld: `2.0.0-next.41` calls `links(router, basePath)` with the router
// context (`dist/claims.js:42-44`), whose `matches()` hold every route record
// and its definition. The arrow could call a `component` it finds there, so
// these definitions prove nothing and each body read stays uncertifiable.
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
  links: (router) => ({
    track: () => router.matches().length,
    pending: () => false,
  }),
});
