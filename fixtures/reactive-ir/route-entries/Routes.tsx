import { onCleanup } from "solid-js";
import { createRouter } from "@solidjs/router";
import { Home } from "./Home";

// Closed: a route component of this file, and one nested in a definition's `children`.
function Layout(props: { children?: unknown }) {
  onCleanup(() => {});
  return <div>{props.children}</div>;
}

function Nested() {
  onCleanup(() => {});
  return <p>nested</p>;
}

// Open: also handed to a timer, which calls it with no owner.
function Shared() {
  onCleanup(() => {});
  return <p>shared</p>;
}
setTimeout(Shared, 0);

// Open: the router calls `component.preload?.()` under the route's root, not as a render; a write of the member is a
// reference no render accounts for.
function Preloaded() {
  onCleanup(() => {});
  return <p>preloaded</p>;
}
(Preloaded as { preload?: () => void }).preload = () => {};

export const Router = createRouter({
  routes: [
    { path: "/", component: Home },
    { path: "/area", component: Layout, children: [{ path: "/inner", component: Nested }] },
    // Closed: an inline arrow's one entry is the router's render.
    {
      path: "/inline",
      component: () => {
        onCleanup(() => {});
        return <p>inline</p>;
      },
    },
    { path: "/shared", component: Shared },
    { path: "/preloaded", component: Preloaded },
  ],
});

export const app = () => <Router />;
