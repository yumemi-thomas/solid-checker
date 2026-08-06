import {
  batch,
  catchError,
  children,
  createContext,
  createEffect,
  createSelector,
  createSignal,
  from,
  lazy,
  on,
  onCleanup,
  useTransition,
} from "solid-js";
import { createDynamic, effect, hydrate, memo, render } from "solid-js/web";
import { modifyMutable, produce } from "solid-js/store";
import { SharedContext } from "./context";
import * as contexts from "./context";
import { renamedMemo, RenamedContext } from "./reexport";
import { CrossFileLazy } from "./lazy-component";

const ValueContext = createContext(0);

function OrdinaryProvider(props: { value: number; children?: JSX.Element }) {
  return <>{props.children}</>;
}

export function ContextValueFromProps(props: {
  contextValue: number;
  children?: JSX.Element;
}) {
  return (
    <ValueContext.Provider value={props.contextValue}>
      {props.children}
    </ValueContext.Provider>
  );
}

export function ContextValueFromSignal() {
  const [contextSignal] = createSignal(0);
  return (
    <ValueContext.Provider value={contextSignal()}>
      context
    </ValueContext.Provider>
  );
}

export function ContextValueFromImported(props: { sharedContextValue: number }) {
  return (
    <SharedContext.Provider value={props.sharedContextValue}>
      imported context
    </SharedContext.Provider>
  );
}

export function ContextValueFromReexport(props: { reexportContextValue: number }) {
  return (
    <RenamedContext.Provider value={props.reexportContextValue}>
      re-exported context
    </RenamedContext.Provider>
  );
}

export function ContextValueFromNamespace(props: { namespaceContextValue: number }) {
  return (
    <contexts.SharedContext.Provider value={props.namespaceContextValue}>
      namespace context
    </contexts.SharedContext.Provider>
  );
}

export function ContextFunctionValueIsDormant() {
  const [dormantContextSignal] = createSignal(0);
  return (
    <ValueContext.Provider value={() => dormantContextSignal()}>
      dormant function
    </ValueContext.Provider>
  );
}

export function MerelyNamedProvider(props: {
  ordinaryValue: number;
  children?: JSX.Element;
}) {
  return (
    <OrdinaryProvider value={props.ordinaryValue}>
      {props.children}
    </OrdinaryProvider>
  );
}

function definedDirective(_element: HTMLDivElement) {}

export function DirectiveNames() {
  return (
    <>
      <div use:definedDirective />
      <div use:missingDirective />
    </>
  );
}

export function DirectContractOnlyReactiveReturn() {
  const externalValue = from<number>(() => () => {});
  console.log(externalValue());
  return <div />;
}

export function DirectContractOnlyCallback() {
  const [renderDependency] = createSignal(0);
  render(() => renderDependency(), document.body);
  return <div />;
}

export function ReexportedContractOnlyReactiveReturn() {
  const reexportedMemoValue = renamedMemo(() => 1);
  console.log(reexportedMemoValue());
  return <div />;
}

export function TupleOnlyReactiveReturn() {
  const [transitionPending] = useTransition();
  console.log(transitionPending());
  return <div />;
}

const [topLevelTransitionRead] = createSignal(0);
const [, startTopLevelTransition] = useTransition();
startTopLevelTransition(() => {
  topLevelTransitionRead();
  onCleanup(() => {});
});

const [trackedTransitionRead, setTrackedTransitionRead] = createSignal(0);
const [, startTrackedTransition] = useTransition();
const startTrackedTransitionAlias = startTrackedTransition;
createEffect(() => {
  startTrackedTransitionAlias(() => {
    trackedTransitionRead();
    setTrackedTransitionRead(1);
    onCleanup(() => {});
  });
});

export function renderOwnsMountedWork() {
  return render(() => {
    createEffect(() => {});
    onCleanup(() => {});
    return <div />;
  }, document.body);
}

export function hydrateOwnsMountedWork() {
  return hydrate(() => {
    createEffect(() => {});
    onCleanup(() => {});
    return <div />;
  }, document.body);
}

// Top-level execution is certainly unowned. These operations are valid only
// because the published web runtime creates a disposal root around each mount
// callback.
render(() => {
  createEffect(() => {});
  onCleanup(() => {});
  return <div />;
}, document.body);

hydrate(() => {
  createEffect(() => {});
  onCleanup(() => {});
  return <div />;
}, document.body);

// Top-level from producers inherit the certainly absent caller owner.
from(() => {
  onCleanup(() => {});
  return () => {};
});

export function FromProducerInheritsComponentOwner() {
  return from(() => {
    onCleanup(() => {});
    return () => {};
  });
}

// `effect` is the web entrypoint's public alias of createRenderEffect. The
// effect callback owns its cleanup, but the top-level effect itself has no
// disposal owner.
effect(() => {
  onCleanup(() => {});
});

const [webMemoSource, setWebMemoSource] = createSignal(0);
const webMemo = memo(() => {
  webMemoSource();
  setWebMemoSource(1);
  onCleanup(() => {});
  return 1;
});
webMemo();

const [dynamicComponentSource, setDynamicComponentSource] = createSignal(0);
createDynamic(() => {
  dynamicComponentSource();
  setDynamicComponentSource(1);
  onCleanup(() => {});
  return "div";
}, {});

const [topLevelBatchSource] = createSignal(0);
batch(() => topLevelBatchSource());

const [trackedBatchSource, setTrackedBatchSource] = createSignal(0);
createEffect(() => {
  batch(() => {
    trackedBatchSource();
    setTrackedBatchSource(1);
  });
});

const [topLevelCatchSource] = createSignal(0);
catchError(
  () => {
    topLevelCatchSource();
    onCleanup(() => {});
  },
  () => {},
);

const [trackedCatchSource, setTrackedCatchSource] = createSignal(0);
createEffect(() => {
  catchError(
    () => {
      trackedCatchSource();
      setTrackedCatchSource(1);
      onCleanup(() => {});
    },
    () => {},
  );
});

const [childrenSource, setChildrenSource] = createSignal(0);
const normalizedChildren = children(() => {
  childrenSource();
  setChildrenSource(1);
  onCleanup(() => {});
  return 1;
});
normalizedChildren();

const [onDependency, setOnDependency] = createSignal(0);
const [onBodySource, setOnBodySource] = createSignal(0);
createEffect(
  on(
    () => {
      onDependency();
      setOnDependency(1);
      return onDependency();
    },
    () => {
      onBodySource();
      setOnBodySource(1);
      onCleanup(() => {});
    },
  ),
);

const [discardedOnSource] = createSignal(0);
on(
  () => discardedOnSource(),
  () => discardedOnSource(),
);

const [discardedProduceSource] = createSignal(0);
produce<{ value: number }>(() => {
  discardedProduceSource();
});

const [invokedProduceSource] = createSignal(0);
modifyMutable(
  { value: 0 },
  produce(draft => {
    invokedProduceSource();
    onCleanup(() => {});
    draft.value = 1;
  }),
);

const [selectorSource] = createSignal(0);
const [discardedSelectorComparatorRead] = createSignal(0);
createSelector(
  () => selectorSource(),
  () => {
    discardedSelectorComparatorRead();
    onCleanup(() => {});
    return true;
  },
);

const [topLevelSelectorComparatorRead] = createSignal(0);
const topLevelSelector = createSelector(
  () => selectorSource(),
  () => {
    topLevelSelectorComparatorRead();
    onCleanup(() => {});
    return true;
  },
);
topLevelSelector("top-level");

const [trackedSelectorComparatorRead, setTrackedSelectorComparatorRead] =
  createSignal(0);
const trackedSelector = createSelector(
  () => selectorSource(),
  () => {
    trackedSelectorComparatorRead();
    setTrackedSelectorComparatorRead(1);
    onCleanup(() => {});
    return true;
  },
);
const trackedSelectorAlias = trackedSelector;
createEffect(() => trackedSelectorAlias("tracked"));

const [discardedLazySource] = createSignal(0);
lazy(async () => {
  discardedLazySource();
  onCleanup(() => {});
  return { default: () => <div /> };
});

const [immediateLazySource] = createSignal(0);
lazy(async () => {
  immediateLazySource();
  onCleanup(() => {});
  return { default: () => <div /> };
})({});

const [jsxLazySource] = createSignal(0);
const JsxLazy = lazy(async () => {
  jsxLazySource();
  onCleanup(() => {});
  return { default: () => <div /> };
});

export function LazyUsedAsComponent() {
  return (
    <>
      <JsxLazy />
      <CrossFileLazy />
    </>
  );
}

const [preloadedLazySource] = createSignal(0);
const PreloadedLazy = lazy(async () => {
  preloadedLazySource();
  onCleanup(() => {});
  return { default: () => <div /> };
});
PreloadedLazy.preload();

// An adapter invoked inside its own factory callback makes execution
// classification cyclic. The cyclic site contributes no context of its own,
// so the read classifies from the remaining top-level invocation — and the
// classifier must terminate rather than recurse through the cycle.
const [cyclicAdapterSource] = createSignal(0);
const cyclicAdapter: () => number = on(
  () => {
    cyclicAdapter();
    return cyclicAdapterSource();
  },
  value => value,
);
cyclicAdapter();

// The two-adapter form of the same cycle. B's only acyclic execution context
// is A's dependency callback, and A's is the tracked effect, so B's read is
// tracked and stays silent.
const [mutualAdapterSource] = createSignal(0);
const mutualAdapterA: () => number = on(
  () => mutualAdapterB(),
  value => value,
);
const mutualAdapterB: () => number = on(
  () => {
    mutualAdapterA();
    return mutualAdapterSource();
  },
  value => value,
);
createEffect(() => mutualAdapterA());
