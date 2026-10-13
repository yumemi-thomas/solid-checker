import {
  createEffect,
  createMemo,
  createOptimisticStore,
  createRoot,
  createSignal,
  createStore,
} from "solid-js";

const [, setCount] = createSignal(0);
const [, setState] = createStore({ value: 0 });
const [, setOptimistic] = createOptimisticStore({ value: 0 });
const [, setDerivedOptimistic] = createOptimisticStore(() => ({ value: 1 }), { value: 0 });

// None on rc.0: an optimistic store's writes take the optimistic engine's path
// and meet no owned-scope guard, in a memo compute or an effect compute.
export const optimisticInMemo = createMemo(() => {
  setOptimistic((draft) => {
    draft.value = 1;
  });
  setDerivedOptimistic((draft) => {
    draft.value = 2;
  });
  return 1;
});
createRoot(() => {
  createEffect(
    () => {
      setOptimistic((draft) => {
        draft.value = 3;
      });
      return 1;
    },
    () => {},
  );
});

// SC2001 on rc.0: a `createStore` setter reaches `setSignal`'s guard, and a
// signal setter meets it directly.
export const storeInMemo = createMemo(() => {
  setState((draft) => {
    draft.value = 4;
  });
  return 1;
});
export const signalInMemo = createMemo(() => {
  setCount(5);
  return 1;
});

// None on rc.0, and not claimed: a store setter the checker knows only by its
// type (an alias binding) could be `createOptimisticStore`'s, which is legal
// here, or `createStore`'s, which throws. The first alias is a correct
// silence, the second a miss. On rc.1-rc.9 both throw, and both are reported.
const aliasedOptimistic = setOptimistic;
const aliasedStore = setState;
export const aliasedInMemo = createMemo(() => {
  aliasedOptimistic((draft) => {
    draft.value = 6;
  });
  aliasedStore((draft) => {
    draft.value = 7;
  });
  return 1;
});
