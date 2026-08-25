#!/usr/bin/env node
// Solid 1.x core probe worker: solid-js and its store/web/jsx-runtime
// entrypoints. Runs inside the solid-v1 install root, so every bare import
// resolves to the audited 1.9.14 release.
//
// Each contracted entrypoint is probed against the binding it actually
// resolves to, never against another entrypoint's. `.` and `./jsx-runtime` are
// the same module object in the browser conditions and different files under
// `node` (`.` selects dist/server.js, jsx-runtime has no node branch and stays
// on dist/solid.js), so probing one and crediting the other would state a
// result no run produced.
//
// 1.x has no flush(): computations run synchronously on write, effects are
// queued. A probe that observes an effect settles with a macrotask.
import * as solid from "solid-js";
import * as jsxRuntime from "solid-js/jsx-runtime";
import * as jsxDevRuntime from "solid-js/jsx-dev-runtime";
import * as store from "solid-js/store";
import * as web from "solid-js/web";

import { createRecorder, describePackages, emit } from "./lib/contract-probe-harness.mjs";

const request = JSON.parse(process.argv[2]);
const mode = request.mode ?? "unspecified";

const packages = await describePackages(request);

const SOLID = "solid-js";
const settle = () => new Promise(resolve => setTimeout(resolve, 0));

// Each probe establishes its own root, because which runtime owns it matters:
// in development `.` resolves to dist/dev.js while ./jsx-runtime stays on
// dist/solid.js, and an effect created in one runtime under a root owned by the
// other belongs to neither scheduler. The recorder therefore does not impose a
// root; `underRoot` builds one from the namespace being probed.
const { probes, probe } = createRecorder({ mode, runInRoot: body => body() });

/** Runs `body` under a disposable root owned by the namespace under probe. */
async function underRoot(S, body) {
  let dispose = () => {};
  try {
    return await S.createRoot(async disposer => {
      dispose = disposer;
      return await body();
    });
  } finally {
    dispose();
  }
}

/** The core entrypoints, each carrying the namespace it resolves to. */
const CORE = [
  [".", solid],
  ["./jsx-runtime", jsxRuntime],
  ["./jsx-dev-runtime", jsxDevRuntime],
];

/** Runs one probe body against every core entrypoint that contracts it. */
async function core(name, claim, body, calls = 1) {
  for (const [entrypoint, namespace] of CORE) {
    await probe(
      SOLID,
      entrypoint,
      name,
      claim,
      () => underRoot(namespace, () => body(namespace)),
      calls,
    );
  }
}

// ---------------------------------------------------------------- inline
// Execution classifies attribution, not timing (see the Execution enum in
// solid-dialect): `inline` means reads inside the callback subscribe whatever
// was tracking at the call site. So an inline probe puts the export call
// inside a memo and asks whether that memo re-runs when a signal the callback
// read changes -- not whether the callback ran before the call returned.
//
// The exception set is the primitives that stay inline while clearing the
// listener, which the dialect states through runs_callback_deferred:
// untrack, createRoot and runWithOwner. For those the observable claim is the
// opposite -- the caller must NOT subscribe -- and they are probed that way.

/**
 * Whether a read inside `call`'s callback subscribes the enclosing computation.
 *
 * The memo is the caller: if it re-runs when the signal changes, the read was
 * attributed to the call site, which is what `inline` asserts.
 */
const attributesToCaller = call => async S => {
  const [source, setSource] = S.createSignal(0);
  let outer = 0;
  const memo = S.createMemo(() => {
    outer++;
    call(S, () => source());
  });
  memo();
  await settle();
  const before = outer;
  setSource(1);
  await settle();
  memo();
  return outer > before;
};

/** An inline claim: reads inside the callback belong to the caller. */
const inlineProbe = (name, call, calls = 2) =>
  core(name, "callbacks[0]=inline", attributesToCaller(call), calls);

/** An inline claim on a listener-clearing primitive: reads do not escape to it. */
const inlineClearingProbe = (name, claim, call, calls = 2) =>
  core(name, claim, async S => !(await attributesToCaller(call)(S)), calls);

await inlineProbe("batch", (S, read) => S.batch(read));

await inlineClearingProbe("untrack", "callbacks[0]=inline", (S, read) => S.untrack(read));

await inlineClearingProbe("createRoot", "callbacks[0]=inline", (S, read) =>
  S.createRoot(dispose => {
    read();
    dispose();
  }),
);

await inlineClearingProbe("createComponent", "callbacks[0]=inline", (S, read) =>
  S.createComponent(read, {}),
);

await inlineProbe("catchError", (S, read) => S.catchError(read, () => {}));

await inlineClearingProbe("runWithOwner", "callbacks[1]=inline", (S, read) =>
  S.runWithOwner(S.getOwner(), read),
);



await inlineProbe("from", (S, read) =>
  S.from(set => {
    read();
    set(1);
    return () => {};
  }),
);

await core("from", "returns=accessor", S => {
  const value = S.from(set => {
    set(7);
    return () => {};
  });
  return typeof value === "function" && value() === 7;
});

// startTransition runs its callback in a microtask and is still inline: the
// runtime restores the captured listener, so the read is the caller's.
await inlineProbe("startTransition", (S, read) => S.startTransition(read));

// ---------------------------------------------------------------- deferred
// `deferred` means the callback does not run while the export call runs.

await core("onCleanup", "callbacks[0]=deferred", S => {
  let called = false;
  S.onCleanup(() => {
    called = true;
  });
  return !called;
});

await core("onMount", "callbacks[0]=deferred", S => {
  let called = false;
  S.onMount(() => {
    called = true;
  });
  return !called;
});

await core("onError", "callbacks[0]=deferred", S => {
  let called = false;
  S.onError(() => {
    called = true;
  });
  return !called;
});

await core("catchError", "callbacks[1]=deferred", S => {
  let handled = false;
  S.catchError(
    () => {},
    () => {
      handled = true;
    },
  );
  return !handled;
});

// The sourced fetcher runs during the createResource call and still does not
// subscribe the caller, which is what deferred asserts.
await core(
  "createResource",
  "callbacks[1]=deferred",
  async S =>
    !(await attributesToCaller((S, read) => {
      const [source] = S.createSignal(1);
      S.createResource(source, () => {
        read();
        return 1;
      });
    })(S)),
  2,
);

await core("createReaction", "callbacks[0]=deferred", S => {
  let called = false;
  S.createReaction(() => {
    called = true;
  });
  return !called;
});

await core("lazy", "callbacks[0]=deferred", S => {
  let called = false;
  S.lazy(() => {
    called = true;
    return Promise.resolve({ default: () => null });
  });
  return !called;
});

await core(
  "requestCallback",
  "callbacks[0]=deferred",
  async S => {
    let inside = true;
    let runs = 0;
    S.requestCallback(() => {
      if (!inside) runs++;
    });
    inside = false;
    await settle();
    return runs === 1;
  },
  2,
);



// ---------------------------------------------------------------- tracked
// `tracked` means the callback re-runs when a value it read changes.

/** Builds a tracked probe for a primitive that takes the tracking function. */
const tracked = (create, read) => async S => {
  const [source, setSource] = S.createSignal(0);
  let runs = 0;
  const result = create(S, () => {
    runs++;
    return source();
  });
  if (read) read(result);
  await settle();
  const before = runs;
  setSource(1);
  await settle();
  if (read) read(result);
  return runs > before;
};

await core("createMemo", "callbacks[0]=tracked", tracked((S, fn) => S.createMemo(fn), memo => memo()), 2);
await core("createMemo", "returns=accessor", S => {
  const memo = S.createMemo(() => 3);
  return typeof memo === "function" && memo() === 3;
});

await core("createComputed", "callbacks[0]=tracked", tracked((S, fn) => S.createComputed(fn)), 2);
await core("createRenderEffect", "callbacks[0]=tracked", tracked((S, fn) => S.createRenderEffect(fn)), 2);
await core("createEffect", "callbacks[0]=tracked", tracked((S, fn) => S.createEffect(fn)), 2);

await core(
  "createDeferred",
  "callbacks[0]=tracked",
  tracked((S, fn) => S.createDeferred(fn, { timeoutMs: 0 }), deferred => deferred()),
  2,
);
await core("createDeferred", "returns=accessor", S => {
  const deferred = S.createDeferred(() => 5, { timeoutMs: 0 });
  return typeof deferred === "function" && deferred() === 5;
});

await core(
  "children",
  "callbacks[0]=tracked",
  tracked((S, fn) => S.children(fn), resolved => resolved()),
  2,
);
await core("children", "returns=accessor", S => {
  const resolved = S.children(() => 9);
  return typeof resolved === "function" && resolved() === 9;
});

await core(
  "createSelector",
  "callbacks[0]=tracked",
  tracked((S, fn) => {
    const selector = S.createSelector(fn);
    selector(0);
    return selector;
  }, selector => selector(0)),
  2,
);
await core("createSelector", "returns=accessor", S => {
  const [source] = S.createSignal(1);
  const selector = S.createSelector(source);
  return typeof selector === "function" && selector(1) === true;
});
await core("createSelector", "callbacks[1]=inline", S => {
  const [source] = S.createSignal(1);
  let called = false;
  const selector = S.createSelector(source, (key, value) => {
    called = true;
    return key === value;
  });
  // The comparator is the selector's own equality test, applied while the
  // selector call runs rather than queued.
  selector(1);
  return called;
});

/** mapArray and indexArray take the list first and the item mapper second. */
const listPrimitive = name =>
  core(
    name,
    "callbacks[0]=tracked",
    // The tracked parameter is the list accessor, so it has to return a list.
    tracked(
      (S, fn) => {
        const mapped = S[name](() => [fn()], item => item);
        mapped();
        return mapped;
      },
      mapped => mapped(),
    ),
    2,
  );
await listPrimitive("mapArray");
await listPrimitive("indexArray");

for (const name of ["mapArray", "indexArray"]) {
  await core(name, "callbacks[1]=deferred", S => {
    let called = false;
    S[name](
      () => [1, 2],
      item => {
        called = true;
        return item;
      },
    );
    return !called;
  });
  await core(name, "returns=accessor", S => {
    const mapped = S[name](
      () => [1, 2],
      item => item,
    );
    return typeof mapped === "function" && Array.isArray(mapped());
  });
}

// ---------------------------------------------------------------- ./store

const rooted = body => () => underRoot(solid, body);

await probe(SOLID, "./store", "createMutable", "returns=store-path", rooted(() => {
  const mutable = store.createMutable({ count: 1 });
  return typeof mutable === "object" && mutable.count === 1;
}));

await probe(SOLID, "./store", "produce", "callbacks[0]=inline", rooted(() => {
  let called = false;
  const recipe = store.produce(draft => {
    called = true;
    draft.count = 2;
  });
  // produce returns the state mutator; the recipe runs when it is applied.
  const [state, setState] = store.createStore({ count: 1 });
  setState(recipe);
  return called && state.count === 2;
}));

await probe(SOLID, "./store", "modifyMutable", "callbacks[1]=inline", rooted(() => {
  let called = false;
  const mutable = store.createMutable({ count: 1 });
  store.modifyMutable(
    mutable,
    store.produce(draft => {
      called = true;
      draft.count = 3;
    }),
  );
  return called && mutable.count === 3;
}));

// ---------------------------------------------------------------- ./web
// render and hydrate mount into a container. A probe has no DOM, and needs
// none: both call the code function while appending, so a container with an
// appendChild is enough to observe the claim without a document.

const container = () => ({
  appendChild() {},
  removeChild() {},
  insertBefore() {},
  querySelectorAll: () => [],
  childNodes: [],
  firstChild: null,
  textContent: "",
  nodeType: 1,
});

// These renderer helpers need no browser global. Keep their probes ahead of
// the legacy render/hydrate shim so their evidence does not depend on it.
await probe(SOLID, "./web", "getNextElement", "callbacks[0]=inline", () => {
  const value = {};
  return web.getNextElement(() => value) === value;
});

await probe(
  SOLID,
  "./web",
  "use",
  "callbacks[0]=inline",
  () =>
    underRoot(solid, async () =>
      !(await attributesToCaller((_S, read) => web.use(read, {}, undefined))(solid)),
    ),
  2,
);

await probe(
  SOLID,
  "./web",
  "createComponent",
  "callbacks[0]=inline",
  () =>
    underRoot(solid, async () =>
      !(await attributesToCaller((_S, read) => web.createComponent(read, {}))(solid)),
    ),
  2,
);

// 1.x render compares its container against the global document before
// inserting, and insert builds nodes through it. Neither claim under probe is
// about the DOM, so the probe supplies the smallest document that lets the
// call proceed rather than pulling in a DOM implementation.
// hydrate reads the hydration global before anything else; without it the call
// throws before the claim can be observed.
globalThis._$HY ??= { done: false, completed: new Set(), events: [], r: {} };

if (typeof globalThis.document === "undefined") {
  globalThis.document = {
    createElement: () => container(),
    createTextNode: () => container(),
    createComment: () => container(),
  };
}

await probe(SOLID, "./web", "render", "callbacks[0]=inline", rooted(() => {
  let called = false;
  const dispose = web.render(() => {
    called = true;
    return null;
  }, container());
  if (typeof dispose === "function") dispose();
  return called;
}));

await probe(SOLID, "./web", "hydrate", "callbacks[0]=inline", () => {
  let called = false;
  const dispose = web.hydrate(() => {
    called = true;
    return null;
  }, container());
  if (typeof dispose === "function") dispose();
  return called;
});

await probe(SOLID, "./web", "untrack", "callbacks[0]=inline", () => {
  let called = false;
  web.untrack(() => {
    called = true;
  });
  return called;
});

/** The ./web probes drive core reactivity through the root namespace. */
const withSolid = body => () => underRoot(solid, () => body(solid));

await probe(
  SOLID,
  "./web",
  "memo",
  "callbacks[0]=tracked",
  withSolid(tracked((_S, fn) => web.memo(fn), memo => memo())),
  2,
);
await probe(SOLID, "./web", "memo", "returns=accessor", () => {
  const memo = web.memo(() => 4);
  return typeof memo === "function" && memo() === 4;
});

await probe(
  SOLID,
  "./web",
  "effect",
  "callbacks[0]=tracked",
  withSolid(tracked((_S, fn) => web.effect(fn))),
  2,
);

await probe(
  SOLID,
  "./web",
  "createDynamic",
  "callbacks[0]=tracked",
  withSolid(
    tracked(
      (_S, fn) => {
        const dynamic = web.createDynamic(() => {
          fn();
          return () => null;
        }, {});
        if (typeof dynamic === "function") dynamic();
        return dynamic;
      },
      dynamic => {
        if (typeof dynamic === "function") dynamic();
      },
    ),
  ),
  2,
);

emit(packages, probes);
