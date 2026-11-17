import { createTrackedEffect, flush } from "solid-js";
import { createRouter, memoryHistory, useNavigate } from "@solidjs/router";

// ADR 1026: `flush()` in a tracked effect is a plain throw (ADR 0893). A top-level route component throws where its
// `<Router>` renders, when the URL matches: reported unless a boundary above takes it. An uncaught throw halts the reactive
// system (REACTIVITY_HALTED), so each route that throws has a case of its own.

// Uncertifiable: the instance's only tag is under an `<Errored>` with a handling fallback (App.tsx).
function GuardedHome() {
  const go = useNavigate();
  return <button id="oracle-guarded" onClick={() => go("/guarded-inline")}>guarded</button>;
}

export const GuardedRouter = createRouter({
  routes: [
    { path: "/", component: GuardedHome },
    {
      path: "/guarded-inline",
      component: () => {
        createTrackedEffect(() => {
          flush();
        });
        return <p>guarded inline</p>;
      },
    },
  ],
  history: memoryHistory(),
});
