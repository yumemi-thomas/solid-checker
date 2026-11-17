import { onCleanup, createSignal } from "solid-js";
import { createRouter } from "@solidjs/router";

const [count] = createSignal(0);

// ADR 1025: a route component of this module, and an inline arrow, each rendered by the router with `createComponent` in the
// route's outlet: their `onCleanup` runs under an owner (no NO_OWNER_CLEANUP), so the checker keeps no owner row. The route table
// is in its own module because the oracle's entry reads `App.tsx` as a namespace, which ADR 0335's premise refuses for the router.
function Layout(props: { children?: unknown }) {
  onCleanup(() => {});
  // A render witness: Chrome warns STRICT_READ_UNTRACKED here only if the router rendered `Layout`.
  const seen = count();
  return <div>{seen}{props.children}</div>;
}

export const Router = createRouter({
  routes: [
    {
      path: "*",
      component: Layout,
      children: [
        {
          path: "*",
          component: () => {
            onCleanup(() => {});
            // The child's render witness.
            const inner = count();
            return <p>{inner}</p>;
          },
        },
      ],
    },
  ],
});
