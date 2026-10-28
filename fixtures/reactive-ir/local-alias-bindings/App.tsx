import {
  action,
  createEffect,
  createMemo,
  createRoot,
  createSignal,
  createStore,
  untrack,
} from "solid-js";

const pause = () => new Promise<void>((resolve) => setTimeout(resolve, 10));

// Violations: an exact `const` alias reads, writes and calls what its source
// does, wherever the direct form is reported.
export function AliasRead() {
  const [r1] = createSignal(3);
  const useIt = r1;
  useIt();
  return <div></div>;
}

export function MemoAliasRead() {
  const m1 = createMemo(() => 1);
  const useIt = m1;
  useIt();
  return <div></div>;
}

export function DestructuredRead() {
  const [r1] = createSignal(3);
  const { useIt } = { useIt: r1 };
  useIt();
  const { "k": keyed } = { k: r1 };
  keyed();
  const { r1: shorthand } = { r1 };
  shorthand();
  return <div></div>;
}

export function ChainedAliasRead() {
  const [r1] = createSignal(3);
  const first = r1;
  const { second } = { second: first };
  second();
  return <div></div>;
}

export function ClosureAliasRead() {
  const [r1] = createSignal(3);
  const { useIt } = { useIt: r1 };
  const read = () => {
    useIt();
  };
  read();
  return <div></div>;
}

export function RootAliasRead() {
  createRoot(() => {
    const [r1] = createSignal(3);
    const useIt = r1;
    useIt();
  });
  return <div></div>;
}

export function EffectApplyAliasRead() {
  const [r1] = createSignal(3);
  const useIt = r1;
  createEffect(
    () => 1,
    () => {
      useIt();
    }
  );
  return <div></div>;
}

export function PendingAsyncAliasRead() {
  const a1 = createMemo(async () => 1);
  const useIt = a1;
  useIt();
  return <div></div>;
}

export function PendingAsyncDestructuredRead() {
  const a1 = createMemo(async () => 1);
  const { useD } = { useD: a1 };
  useD();
  return <div></div>;
}

export function AfterAwaitAliasRead() {
  const [r1] = createSignal(3);
  const useIt = r1;
  const late = createMemo(async () => {
    await pause();
    return useIt();
  });
  return <div>{late()}</div>;
}

export function DestructuredSignalWrite() {
  const [w1, setW1] = createSignal(0);
  const { useIt } = { useIt: setW1 };
  useIt(7);
  return <div>{w1()}</div>;
}

export function DestructuredStoreWrite() {
  const [s1, setS1] = createStore({ v: 0 });
  const { useSet } = { useSet: setS1 };
  useSet((state) => {
    state.v = 7;
  });
  return <div>{s1.v}</div>;
}

export function StoreAliasMutation() {
  const [s1] = createStore({ v: 0 });
  const useIt = s1;
  useIt.v = 7;
  return <div></div>;
}

export function DestructuredAction() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () {
    setW1(7);
  });
  const { useIt } = { useIt: a1 };
  useIt();
  return <div>{w1()}</div>;
}

// Uncertifiable: a mention that may run before the alias's declarator throws
// `ReferenceError` instead of reading or writing.
export function BeforeDeclarator() {
  const [r1] = createSignal(3);
  const [, setW1] = createSignal(0);
  [1].forEach(() => useLate());
  [1].forEach(() => setLate(7));
  const useLate = r1;
  const { setLate } = { setLate: setW1 };
  return <div></div>;
}

// Silent: tracked scopes, untrack, a non-reactive source, and a key the
// computation read before its await through the source or the alias.
export function TrackedAliasReads() {
  const [r1] = createSignal(3);
  const useIt = r1;
  const { useD } = { useD: r1 };
  const memo = createMemo(() => useIt() + useD());
  const peek = untrack(() => useIt());
  const plain = () => 1;
  const usePlain = plain;
  usePlain();
  const [s1] = createStore({ a: 1 });
  const { view } = { view: s1 };
  const before = createMemo(async () => {
    useIt();
    s1.a;
    await pause();
    return r1() + view.a;
  });
  return <div>{useIt()}{memo()}{peek}{before()}</div>;
}

// Refused: no exact alias, so no read. A `let`, a shadowing name, a
// parameter, a literal with a spread, a computed key or a getter, and a
// default. Where the type says `Accessor`, the call stays the
// typed dispatch obligation (SC9012) it was before (ADR 0294).
export function Refused() {
  const [r1] = createSignal(3);
  const [r2] = createSignal(4);
  let useLet = r1;
  useLet();
  const shadow = () => {
    const r1 = () => 1;
    const useShadow = r1;
    useShadow();
  };
  shadow();
  const viaParam = (accessor: () => number) => {
    const useParam = accessor;
    useParam();
  };
  viaParam(r1);
  const { spread } = { spread: r1, ...{} };
  spread();
  const { computed } = { ["computed"]: r1 };
  computed();
  const { got } = {
    got: r1,
    get other() {
      return 1;
    },
  };
  got();
  const { defaulted = r1 } = { defaulted: r2 };
  defaulted();
  return <div></div>;
}

// Refused: a conditional alias holds one of two sources; its call stays the
// typed dispatch obligation it was (SC9012), never a read.
export function ConditionalAlias() {
  const [r1] = createSignal(3);
  const [r2] = createSignal(4);
  const useCond = Math.random() > 0.5 ? r1 : r2;
  useCond();
  return <div></div>;
}
