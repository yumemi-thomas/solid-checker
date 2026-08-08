import {
  createEffect,
  createReaction,
  createResource,
  createSignal,
  indexArray,
  mapArray,
  onCleanup,
} from "solid-js";
import { createStore } from "solid-js/store";

// createResource's callback contract is overload-sensitive. With one
// argument, index 0 is an untracked fetcher; with two, index 0 is the tracked
// source and index 1 is the untracked fetcher.
export function ResourceCallbackOverloads() {
  const [fetcherDependency] = createSignal(1);
  createResource(() => fetcherDependency());

  const [sourceDependency] = createSignal(1);
  createResource(() => sourceDependency(), async value => value);
}

export function StoredFunctionShapes() {
  const [nestedDormantSource] = createSignal(1);
  createSignal(() => () => nestedDormantSource());

  const [evaluatedSource] = createSignal(1);
  createSignal((() => evaluatedSource())());
}

export function ArrayCallbackContracts() {
  const [items] = createSignal([{ value: 1 }]);
  const [mappingStore] = createStore({ path: { to: { field: 1 } } });

  const mapped = mapArray(items, (_item, mapIndex) => {
    mapIndex();
    return mappingStore.path.to.field;
  });
  const indexed = indexArray(items, indexedItem => indexedItem().value);
  createEffect(mapped);
  createEffect(indexed);

  const [trackedListStore] = createStore({ items: [1] });
  const tracked = mapArray(() => trackedListStore.items, item => item);
  createEffect(tracked);
}

export function DiscardedArrayAdaptersAreDormant() {
  const [items] = createSignal([{ value: 1 }]);
  const [discardedMapperStore] = createStore({ path: 1 });

  mapArray(items, (_item, discardedMapIndex) => {
    discardedMapIndex();
    return discardedMapperStore.path;
  });
  indexArray(items, discardedIndexedItem => discardedIndexedItem().value);
}

export function ArrayListInvocationContexts() {
  const [untrackedListStore] = createStore({ items: [1] });
  const untrackedAdapter = mapArray(() => untrackedListStore.items, item => item);
  untrackedAdapter();

  const [immediateListStore] = createStore({ items: [1] });
  mapArray(() => immediateListStore.items, item => item)();

  const [trackedImmediateListStore] = createStore({ items: [1] });
  createEffect(() =>
    mapArray(() => trackedImmediateListStore.items, item => item)(),
  );

  const [mixedListStore] = createStore({ items: [1] });
  const mixedAdapter = indexArray(() => mixedListStore.items, item => item);
  createEffect(mixedAdapter);
  mixedAdapter();
}

// A member call off the factory result invokes the member, not the returned
// adapter: toString never runs the list or the mapper, so neither a
// mapped-array read nor the mapper's store read may be recorded.
export function MemberCalleesAreNotAdapterInvocations() {
  const [memberListStore] = createStore({ items: [1] });
  const [memberMapperStore] = createStore({ path: 1 });
  mapArray(
    () => memberListStore.items,
    () => memberMapperStore.path,
  ).toString();
}

export function ReactionInvalidationReachability() {
  const [reactionSource, setReactionSource] = createSignal(0);
  const trackReaction = createReaction(() => {
    onCleanup(() => {});
  });
  trackReaction(() => reactionSource());
  setReactionSource(1);

  const [aliasedReactionSource, setAliasedReactionSource] = createSignal(0);
  const originalTracker = createReaction(() => {});
  const aliasedTracker = originalTracker;
  const namedTrackingCallback = () => {
    aliasedReactionSource();
    setAliasedReactionSource(1);
    onCleanup(() => {});
  };
  aliasedTracker(namedTrackingCallback);

  createReaction(() => {
    onCleanup(() => {});
  });
}
