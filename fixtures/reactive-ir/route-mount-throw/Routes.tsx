import { Errored, createTrackedEffect, flush } from "solid-js";
import { createRouter } from "@solidjs/router";

// `flush()` in a tracked effect is a plain throw (rc.21 "Cannot call flush() from inside onSettled or createTrackedEffect",
// ADR 0893): it reports where its component renders unless a boundary above takes it.

// Violations (ADR 1026): a top-level route definition's `component`, a bare function and an inline arrow. The router
// renders each where the URL matches, at the `<Router>` tag, with no boundary above it.
function First() {
  createTrackedEffect(() => {
    flush();
  });
  return <p>first</p>;
}

export const Router = createRouter({
  routes: [
    { path: "/first", component: First },
    {
      path: "/second",
      component: () => {
        createTrackedEffect(() => {
          flush();
        });
        return <p>second</p>;
      },
    },
  ],
});

// Obligations (uncertifiable): a boundary above the only `<Router>` tag takes the throw.
function Guarded() {
  createTrackedEffect(() => {
    flush();
  });
  return <p>guarded</p>;
}

const GuardedRouter = createRouter({
  routes: [
    { path: "/guarded", component: Guarded },
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
});

// Violations: a child route renders where its parent reads `props.children` (the route props' `children` getter calls the
// outlet), here in the parent's JSX with no boundary above.
function Parent(props: { children?: unknown }) {
  return <div>{props.children}</div>;
}

function Child() {
  createTrackedEffect(() => {
    flush();
  });
  return <p>child</p>;
}

const NestedRouter = createRouter({
  routes: [
    {
      path: "/parent",
      component: Parent,
      children: [
        { path: "/child", component: Child },
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
});

// Obligations: a parent that takes the child's throw (`<Errored>` around `props.children`), and a parent that hands its
// props on (the child renders wherever `Frame` reads them, which this reading does not follow).
function CatchingParent(props: { children?: unknown }) {
  return <Errored fallback={(error) => <p>{String(error())}</p>}>{props.children}</Errored>;
}

function CaughtChild() {
  createTrackedEffect(() => {
    flush();
  });
  return <p>caught child</p>;
}

function Frame(props: { children?: unknown }) {
  return <div>{props.children}</div>;
}

function HandingParent(props: { children?: unknown }) {
  return <Frame {...props} />;
}

function HandedChild() {
  createTrackedEffect(() => {
    flush();
  });
  return <p>handed child</p>;
}

const ParentsRouter = createRouter({
  routes: [
    { path: "/catching", component: CatchingParent, children: [{ path: "/caught", component: CaughtChild }] },
    { path: "/handing", component: HandingParent, children: [{ path: "/handed", component: HandedChild }] },
  ],
});

// An obligation: a table held in a binding is not read here.
function Held() {
  createTrackedEffect(() => {
    flush();
  });
  return <p>held</p>;
}

const table = [{ path: "/held", component: Held }];
const HeldRouter = createRouter({ routes: table });

// A router instance no tag renders mounts nothing.
function Unmounted() {
  createTrackedEffect(() => {
    flush();
  });
  return <p>unmounted</p>;
}

export const UnmountedRouter = createRouter({ routes: [{ path: "/unmounted", component: Unmounted }] });

export const app = () => (
  <div>
    <Router />
    <Errored fallback={(error) => <p>{String(error())}</p>}>
      <GuardedRouter />
    </Errored>
    <NestedRouter />
    <ParentsRouter />
    <HeldRouter />
  </div>
);
