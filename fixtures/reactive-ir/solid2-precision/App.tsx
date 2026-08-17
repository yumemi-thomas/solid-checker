import {
  createEffect,
  createMemo,
  createSignal,
  createStore,
  onCleanup,
  onSettled,
} from "solid-js";

declare function fetchPosts(): Promise<string[]>;
declare function invoke<T, U>(value: T, callback: (value: T) => U): U;
declare function makePredicate(
  callback: (post: string) => boolean,
): (post: string) => boolean;
declare function wrap(callback: () => void): () => void;
declare const unknownValue: unknown;
declare const anyResult: any;
declare const nothing: undefined;
declare const incoming: { name?: string };
declare function makeCount(): number;
declare function makeThunk(): () => void;
declare function makeMaybeThunk(): (() => void) | undefined;
declare function makeNothing(): void;
declare function makeAny(): any;
declare const handlers: Array<() => () => void>;
declare const index: number;
declare const teardown: {
  dispose: () => void;
  count: number;
  optional?: () => void;
  mixed: (() => void) | number;
  anything: any;
};
declare const teardownKey: "dispose";

// Positive: Array.prototype.filter is an exact synchronous callback target.
// The accessor read is after the dominating await, but still runs before the
// async computation resumes because filter invokes its callback inline.
export function FilterAfterAwait() {
  const [id] = createSignal("42");
  const posts = createMemo(async () => {
    const result = await fetchPosts();
    return result.filter((post) => post.includes(id()));
  });
  return <div>{posts()}</div>;
}

// Promise callbacks are deferred and must not inherit Array.prototype.filter's
// synchronous proof merely because a filter appears inside them.
export function PromiseCallbackAfterAwait() {
  const [id] = createSignal("42");
  const posts = createMemo(async () => {
    const result = await fetchPosts();
    return Promise.resolve(result).then((values) => values.filter((post) => post.includes(id())));
  });
  return <div>{posts()}</div>;
}

// This is a project-defined method with the same spelling as the built-in.
// Name-only or regex matching would incorrectly certify its callback.
export function ShadowedFilterAfterAwait() {
  const [id] = createSignal("42");
  const posts = createMemo(async () => {
    const result = await fetchPosts();
    const collection = {
      filter(callback: (post: string) => boolean) {
        return callback("42");
      },
    };
    return collection.filter((post) => post.includes(id()));
  });
  return <div>{posts()}</div>;
}

// A store member read inside the proven-synchronous callback is the awaiting
// computation's own read, exactly like an accessor call there.
export function FilterStoreReadAfterAwait() {
  const [selection] = createStore({ id: "42" });
  const posts = createMemo(async () => {
    const result = await fetchPosts();
    return result.filter((post) => post === selection.id);
  });
  return <div>{posts()}</div>;
}

// `filter` receives whatever `makePredicate` returns, not the arrow written
// here: the wrapper may stash it and run it under a later tracking scope. The
// callback must be the literal argument, not merely the largest function
// somewhere inside the argument text.
export function WrappedFilterCallbackAfterAwait() {
  const [id] = createSignal("42");
  const posts = createMemo(async () => {
    const result = await fetchPosts();
    return result.filter(makePredicate((post) => post.includes(id())));
  });
  return <div>{posts()}</div>;
}

// An unresolved/user-defined callback remains outside the synchronous proof.
export function UnresolvedCallbackAfterAwait() {
  const [id] = createSignal("42");
  const posts = createMemo(async () => {
    const result = await fetchPosts();
    return invoke(result, (post) => post.includes(id()));
  });
  return <div>{posts()}</div>;
}

// Contextual inference and an explicit primitive return are both provable
// invalid cleanup values in Solid 2's second effect callback.
export function PrimitiveCleanupReturns() {
  createEffect(() => 123, (value) => value);
  createEffect<number>(() => 123, (value): number => value);
  createEffect(() => 123, () => undefined);
  createEffect(() => 123, () => () => {});
  createEffect(() => 123, () => unknownValue);
  createEffect(() => 123, () => anyResult);
  return <div />;
}

// The same proof has to survive the wrappers around the returned expression.
// Parentheses and `as` casts are peeled to the identifier, so the demanded
// fact and the classification have to name that same span.
export function WrappedPrimitiveCleanupReturns() {
  createEffect(() => 123, (value) => (value));
  createEffect(() => 123, (value) => value as number);
  createEffect(() => 123, (value) => {
    return (value);
  });
  return <div />;
}

// Static member returns use the complete member expression's value domain.
// A function-valued member is a valid cleanup, while a proven primitive is
// SC3004. Optional/union, any, and computed-member dispatch remain unresolved.
export function MemberCleanupReturns() {
  createEffect(() => 123, () => teardown.dispose);
  createEffect(() => 123, () => {
    return teardown.count;
  });
  createEffect(() => 123, () => teardown.optional);
  createEffect(() => 123, () => teardown.mixed);
  createEffect(() => 123, () => teardown.anything);
  createEffect(() => 123, () => teardown[teardownKey]);
  return <div />;
}

export function GenericCleanup<T>(value: T) {
  createEffect(() => 123, () => value);
  return <div />;
}

// A returned call is classified from what the call *produces*, never from its
// callee. Every callee here is itself callable, so a callee-shaped fact would
// certify all of them as cleanups; only the ones whose result is a function
// hand the owner anything it can call. Both return spellings are pinned,
// because an expression-bodied arrow records its return on the function fact
// rather than in the statement list.
export function ReturnedCallCleanupReturns() {
  createEffect(() => 123, () => makeCount());
  createEffect(() => 123, () => {
    return makeCount();
  });
  createEffect(() => 123, () => makeThunk());
  createEffect(() => 123, () => makeMaybeThunk());
  createEffect(() => 123, () => makeNothing());
  createEffect(() => 123, () => makeAny());
  createEffect(() => 123, () => handlers[index]());
  return <div />;
}

// A direct assignment writes without reading the old store value. Compound
// assignment and update expressions do read it and retain SC1001.
export function StoreAssignmentTargets() {
  const [profile] = createStore({ name: "Ada", count: 0 });
  profile.name = "Grace";
  profile.count += 1;
  profile.count++;
  const snapshot = profile.name;
  return <div>{snapshot}</div>;
}

// Only the member that IS the written target is a write. A computed key and a
// destructuring default are evaluated to address or build the value, so the
// reads inside them survive.
export function ComputedAssignmentTarget(props: { index: number }) {
  const [rows] = createStore([{ done: false }]);
  rows[props.index].done = true;
  return <div>{String(rows[0].done)}</div>;
}

export function DestructuredAssignmentDefault() {
  const [fallback] = createStore({ name: "Ada" });
  let local = "";
  ({ name: local = fallback.name } = incoming);
  return <div>{local}</div>;
}

function dispose(): void {}

// Owner-backed onSettled is a leaf scope: onCleanup is SC3001, not also SC4002.
export function OwnedSettledCleanup() {
  onSettled(() => {
    onCleanup(dispose);
  });
  return <div />;
}

// `wrap` may stash the callback and run it out-of-band, so the callback the
// owner receives is not the arrow written here. No leaf scope is proven where
// the onCleanup is written, so it is not SC3001; the cleanup stays genuinely
// unowned and keeps SC4002.
export function WrappedSettledCleanup() {
  onSettled(
    wrap(() => {
      onCleanup(dispose);
    }),
  );
  return <div />;
}

// The callback reaches the owner as an identifier reference, so the leaf pass
// has no literal argument to scan and stays silent — `settledCleanup` has
// other, unowned callers this pass cannot see. The onCleanup keeps SC4002.
const settledCleanup = () => {
  onCleanup(dispose);
};
onSettled(settledCleanup);

// Literal callback, but the onCleanup sits in a nested function the callback
// only builds. Calling `onSettled` executes nothing in it, so it is not in the
// leaf scope's synchronous extent and reports no SC3001. Nothing else fires
// either: a nested function the analysis never sees invoked proves no owner
// context, exactly as `leaf-owner`'s `buildTeardownHandler` does.
export function NestedSettledCleanup() {
  onSettled(() => {
    const later = () => {
      onCleanup(dispose);
    };
    void later;
  });
  return <div />;
}

// Out-of-band onSettled does not materialize a leaf owner. Its onCleanup is
// genuinely unowned and remains SC4002.
onSettled(() => {
  onCleanup(dispose);
});

// An unowned returned cleanup remains SC4004.
onSettled(() => () => {});

// A returned call read as its result. `makeCount` is callable, so classifying
// the callee registered a cleanup here and reported a false SC4004 on this
// unowned callback; the call produces a number, which is no cleanup at all —
// SC3004, and no SC4004. `makeThunk` does produce one, so its unowned callback
// still reports SC4004.
onSettled(() => {
  return makeCount();
});
onSettled(() => makeThunk());

// `nothing` is provably `undefined`: a legal cleanup return that hands the
// owner no cleanup at all. Reading "valid" as "returned a cleanup function"
// would make both of these a false SC4004, and neither is SC3004 or SC9002.
onSettled(() => {
  return nothing;
});
onSettled(() => nothing);
