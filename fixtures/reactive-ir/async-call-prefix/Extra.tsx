import { createEffect, createSignal, createStore } from "solid-js";
import type { Accessor, Store } from "solid-js";

export function StoreProtocolKeys() {
  const marker = Symbol("plain");
  const [state] = createStore({ then: 1, value: 2, [marker]: 3 });
  const key: "then" | "value" = "then";
  async function thenBody() { return state.then; }
  async function symbolBody() { return state[marker]; }
  async function dynamicBody() { return state[key]; }
  async function thenDefault(next = state["th\u0065n"]) { return next; }
  async function symbolDefault(next = state[marker]) { return next; }
  async function dynamicDefault(next = state[key]) { return next; }
  async function cookedDefault(next = state["val\u0075e"]) { return next; }
  createEffect(() => 1, () => {
    void thenBody(); void symbolBody(); void dynamicBody();
    void thenDefault(); void symbolDefault(); void dynamicDefault();
    void cookedDefault(); // Violation: known cooked ordinary string Get.
  });
  return <p>store keys</p>;
}

export function TypeOnlyInputs() {
  const read: Accessor<number> = () => 1;
  const plain: Store<{ range: { value: number } }> = { range: { value: 1 } };
  async function helper(next = read(), range = plain.range) { return next + range.value; }
  createEffect(() => 1, () => { void helper(); }); // No runtime-source witness.
  return <p>plain</p>;
}

export function ReplacedSourceDefault() {
  const [original] = createStore({ range: { value: 1 } });
  let state: Store<{ range: { value: number } }> = original;
  state = { range: { value: 2 } };
  async function helper(range = state.range) { return range.value; }
  createEffect(() => 1, () => { void helper(); }); // No new default violation.
  return <p>replaced source</p>;
}

export function ReassignedTarget() {
  const [count] = createSignal(1);
  let helper = async (): Promise<number> => count();
  helper = async () => 0;
  void helper(); // No prefix promotion from the original function declaration.
  return <p>replaced</p>;
}

export function SuspensionFreeLoop() {
  const [count] = createSignal(1);
  async function helper() {
    for (let i = 0; i < 2; i++) { void count(); }
  }
  void helper(); // Violation: no suspension on any loop iteration.
  return <p>prefix</p>;
}

export function BeforeImplicitSuspension() {
  const [count] = createSignal(1);
  async function helper() {
    const value = count(); // Violation: before entering the for-await region.
    for await (const item of [1]) { void item; }
    return value;
  }
  void helper();
  return <p>prefix</p>;
}

export function DefaultOnlyWrites() {
  const [state] = createStore({ value: 1, box: { value: 1 } });
  async function helper(next = (state.value = 2)) { return next; }
  createEffect(() => 1, () => { void helper(); }); // No default Get of value.
  return <p>write</p>;
}

export function DefaultDeletes() {
  const [state] = createStore<{ value?: number }>({ value: 1 });
  async function helper(next = delete state.value) { return next; }
  createEffect(() => 1, () => { void helper(); }); // No default Get of value.
  return <p>delete</p>;
}

export function PrefixThroughAsyncChild() {
  const [count] = createSignal(1);
  async function child() { const value = count(); await Promise.resolve(); return value; }
  async function parent() { const result = child(); await Promise.resolve(); return result; }
  createEffect(() => 1, () => { void parent(); }); // Violation, both edges/prefixes proven.
  return <p>chain</p>;
}

export function ConditionalSuspendBeforeChild() {
  const [count] = createSignal(1);
  function child() { return count(); }
  async function parent(later: boolean) {
    if (later) await Promise.resolve();
    return child();
  }
  createEffect(() => 1, () => { void parent(true); }); // Uncertifiable, not direct.
  return <p>detached</p>;
}

export function MultipleRoles() {
  const [count] = createSignal(1);
  async function helper() { const value = count(); await Promise.resolve(); return value; }
  createEffect(() => helper(), () => {}); // No SC1001 in tracked compute.
  createEffect(() => 1, () => { void helper(); }); // SC1001 violation at this occurrence.
  return <button onClick={() => { void helper(); }}>event</button>; // No SC1001 on dispatch.
}
