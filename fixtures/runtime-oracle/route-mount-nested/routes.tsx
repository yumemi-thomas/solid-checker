import { createTrackedEffect, flush } from "solid-js";
import { createRouter, memoryHistory, useNavigate } from "@solidjs/router";

// ADR 1026: `flush()` in a tracked effect is a plain throw (ADR 0893). A top-level route component throws where its
// `<Router>` renders, when the URL matches: reported unless a boundary above takes it. An uncaught throw halts the reactive
// system (REACTIVITY_HALTED), so each route that throws has a case of its own.

// Uncertifiable: a child route renders where its parent reads `props.children`, which the reading does not follow.
function Parent(props: { children?: unknown }) {
  const go = useNavigate();
  return (
    <div>
      <button id="oracle-child" onClick={() => go("/parent/child-inline")}>child</button>
      {props.children}
    </div>
  );
}

export const NestedRouter = createRouter({
  routes: [
    {
      path: "/parent",
      component: Parent,
      children: [
        { path: "/", component: () => <p>index</p> },
        {
          path: "/child-inline",
          component: () => {
            createTrackedEffect(() => {
              flush();
            });
            return <p>child inline</p>;
          },
        },
      ],
    },
  ],
  history: memoryHistory("/parent"),
});
