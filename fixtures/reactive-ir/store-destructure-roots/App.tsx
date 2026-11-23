import { createMemo, createStore, merge } from "solid-js";

// ADR 1193: a store destructure whose root is no plain identifier initializer. `destructure_root` names the root where the runtime
// reads every key from the store exactly: the sole element of the array literal a `for…of` iterates, and the first source of
// `merge(s, {…})` when no later literal source has a key the pattern reads. SC1003 claims it in setup as for `const { v } = s`;
// elsewhere SC1001 places each key's Get.

// Violations (SC1003): read once in the component body (Chrome: STRICT_READ_UNTRACKED).
export function ForOfBody() {
  const [s1] = createStore({ v: 0 });
  for (const { v } of [s1]) void v;
  return <i />;
}
export function MergeBody() {
  const [s1] = createStore({ v: 0 });
  const { v } = merge(s1, { w: 1 });
  void v;
  return <i />;
}

// Clean: the same in a memo's compute, which tracks its reads.
export function ForOfMemo() {
  const [s1] = createStore({ v: 0 });
  const m = createMemo(() => {
    for (const { v } of [s1]) return v;
    return 0;
  });
  return <i>{m()}</i>;
}
export function MergeMemo() {
  const [s1] = createStore({ v: 0 });
  const m = createMemo(() => {
    const { v } = merge(s1, { w: 1 });
    return v;
  });
  return <i>{m()}</i>;
}

// Not this proof (uncertifiable, as before): two elements, a later source that has the key (it answers `v` itself), a non-literal
// later source, a `for await`.
export function ForOfTwo() {
  const [s1] = createStore({ v: 0 });
  const [s2] = createStore({ v: 1 });
  for (const { v } of [s1, s2]) void v;
  return <i />;
}
export function MergeShadowed() {
  const [s1] = createStore({ v: 0 });
  const { v } = merge(s1, { v: 1 });
  void v;
  return <i />;
}
export function MergeOpen(props: { extra: { w: number } }) {
  const [s1] = createStore({ v: 0 });
  const { v } = merge(s1, props.extra);
  void v;
  return <i />;
}
