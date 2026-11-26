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

// ADR 1193 part 2 (facts schema 110): a plain assignment's object target, and an object pattern that is an element of a declarator's
// array pattern over a literal. Violations (SC1003) in the component body (Chrome: STRICT_READ_UNTRACKED).
export function AssignBody() {
  const [s1] = createStore({ v: 0 });
  let v = 0;
  ({ v } = s1);
  void v;
  return <i />;
}
export function AssignMergeBody() {
  const [s1] = createStore({ v: 0 });
  let v = 0;
  ({ v } = merge(s1, { w: 1 }));
  void v;
  return <i />;
}
export function AssignMember() {
  const [s1] = createStore({ v: 0 });
  const o = { x: 0 };
  ({ v: o.x } = s1);
  return <i>{o.x}</i>;
}
export function ArrayBody() {
  const [s1] = createStore({ v: 0 });
  const [{ v }] = [s1];
  void v;
  return <i />;
}
export function ArraySecond() {
  const [s1] = createStore({ v: 0 });
  const [, { v }] = [0, s1];
  void v;
  return <i />;
}

// Clean: the same in a memo's compute.
export function AssignMemo() {
  const [s1] = createStore({ v: 0 });
  const m = createMemo(() => {
    let v = 0;
    ({ v } = s1);
    return v;
  });
  return <i>{m()}</i>;
}
export function ArrayMemo() {
  const [s1] = createStore({ v: 0 });
  const m = createMemo(() => {
    const [{ v }] = [s1];
    return v;
  });
  return <i>{m()}</i>;
}

// Not this proof (uncertifiable, as before): a hole before the element, a spread in the literal, a props root.
export function ArrayHole() {
  const [s1] = createStore({ v: 0 });
  // eslint-disable-next-line no-sparse-arrays
  const [, { v }] = [, s1];
  void v;
  return <i />;
}
export function ArraySpread() {
  const [s1] = createStore({ v: 0 });
  const [{ v }] = [...[s1]];
  void v;
  return <i />;
}
export function AssignProps(props: { a: number }) {
  let a = 0;
  ({ a } = props);
  return <i>{a}</i>;
}
