import { createEffect, createSignal, createStore, untrack } from "solid-js";
import { remotePrefix, remoteSuffix } from "./remote";

const [count] = createSignal(1);

async function prefix() {
  const value = count(); // PREFIX: direct origin, before suspension.
  await Promise.resolve();
  return value;
}

async function suffix() {
  await Promise.resolve();
  return count(); // SUFFIX: never inherits the invocation's role.
}

export function BeforeAwaitBody() {
  void prefix(); // violation: PREFIX.
  return <main />;
}

export function BeforeAwaitApply() {
  createEffect(() => 1, () => { void prefix(); }); // violation: PREFIX.
  return <main />;
}

export function AfterAwaitBody() {
  void suffix(); // uncertifiable summary, not a caller-role violation.
  return <main />;
}

export function AfterConditionalAwait() {
  async function read(later: boolean) {
    if (later) await Promise.resolve();
    return count();
  }
  void read(true); // uncertifiable: a possible suspension is enough.
  return <main />;
}

export function LoopThenConditionalAwait() {
  async function read(later: boolean) {
    for (let i = 0; i < 2; i++) await Promise.resolve();
    const value = count(); // before the next await, but not in the prefix.
    if (later) await Promise.resolve();
    return value;
  }
  void read(false); // uncertifiable.
  return <main />;
}

export function LoopBackEdge() {
  async function read() {
    for (let i = 0; i < 2; i++) {
      void count(); // lexically before await, repeated after suspension.
      await Promise.resolve();
    }
  }
  void read(); // uncertifiable under the all-occurrences prefix proof.
  return <main />;
}

export function OmittedDefaultApply() {
  const [value] = createSignal(1);
  async function read(next = value()) { // DEFAULT: activated at read().
    await Promise.resolve(next);
  }
  createEffect(() => 1, () => { void read(); }); // violation: DEFAULT.
  return <main />;
}

export function SuppliedDefaultApply() {
  const [value] = createSignal(1);
  async function read(next = value()) {
    await Promise.resolve(next);
  }
  createEffect(() => 2, next => { void read(next); }); // no new violation.
  return <main />;
}

export function StoreDefaultApply() {
  const [state] = createStore({ range: { agentId: "agent" } });
  async function load(range = state.range) { // STORE_DEFAULT: real Get.
    await Promise.resolve(range.agentId);
  }
  createEffect(() => 1, () => { void load(); }); // violation: STORE_DEFAULT.
  return <main />;
}

export function SuppliedStoreDefaultApply() {
  const [state] = createStore({ range: { agentId: "agent" } });
  async function load(range = state.range) {
    const agentId = range.agentId; // no global store taint from the default.
    await Promise.resolve(agentId);
  }
  createEffect(
    () => ({ agentId: state.range.agentId }),
    range => { void load(range); }, // no new violation.
  );
  return <main />;
}

export function SampledApply() {
  createEffect(() => 1, () => { void untrack(() => prefix()); }); // clean.
  return <main />;
}

export function SampledDefaultApply() {
  async function read(next = count()) { await Promise.resolve(next); }
  createEffect(() => 1, () => { void untrack(() => read()); }); // no new violation.
  return <main />;
}

export function TrackedCompute() {
  createEffect(() => prefix(), () => {}); // prefix is tracked.
  return <main />;
}

function innerRead() { return count(); }
async function outerPrefix() {
  const value = innerRead();
  await Promise.resolve();
  return value;
}
async function outerSuffix() {
  await Promise.resolve();
  return innerRead();
}

export function NestedHelperPrefix() {
  void outerPrefix(); // violation: every edge is in the prefix.
  return <main />;
}

export function NestedHelperAfterAwait() {
  void outerSuffix(); // uncertifiable: innerRead is reached after suspension.
  return <main />;
}

export function DormantNestedAwait() {
  async function read() {
    const later = async () => { await Promise.resolve(); };
    const value = count(); // violation; later's await is not this body's.
    void later;
    return value;
  }
  void read();
  return <main />;
}

export function DormantNestedDefault() {
  async function read() {
    function later(next = count()) { return next; }
    return later; // constructing a closure/default does not read count.
  }
  void read(); // no caller-role violation.
  return <main />;
}

export function LazyDefault() {
  async function read(next = () => count()) { return next; }
  void read(); // omitted default constructs a closure; no Get/Call of count.
  return <main />;
}

export function AsyncGenerator() {
  async function* read() { yield count(); }
  void read(); // no stepping, no body execution proof.
  return <main />;
}

export function AwaitOperand() {
  async function read() { return await Promise.resolve(count()); }
  void read(); // intentionally uncertifiable in this conservative slice.
  return <main />;
}

export function ForAwait() {
  async function read() {
    for await (const item of [1]) { void item; void count(); }
  }
  void read(); // implicit suspension before the body: uncertifiable.
  return <main />;
}

export function AwaitUsing() {
  async function read() {
    { await using resource = { [Symbol.asyncDispose]: async () => {} }; }
    return count(); // disposal may have suspended.
  }
  void read(); // uncertifiable.
  return <main />;
}

export function DestructureAfterAwait() {
  async function read() {
    const { value = count() } = await Promise.resolve<{ value?: number }>({});
    return value; // the textual default precedes await; evaluation does not.
  }
  void read(); // uncertifiable.
  return <main />;
}

export function SwitchDefaultAfterTest() {
  async function read(value: number) {
    switch (value) {
      default: return count();
      case await Promise.resolve(2): return 0;
    }
  }
  void read(1); // uncertifiable: later-written case test suspends first.
  return <main />;
}

export function PreferPrefixOrigin() {
  async function read() {
    const value = count(); // direct origin must win per-call symbol dedup.
    await Promise.resolve();
    return count() + value;
  }
  void read(); // one violation whose origin is the FIRST count call.
  return <main />;
}

async function topLevelDefault(value = count()) {
  await Promise.resolve(value);
}

export function TopLevelOmittedDefault() {
  void topLevelDefault(); // violation with the authored count() origin.
  return <main />;
}

export function DefaultActivationNotProven(props: { value: number | undefined }) {
  void topLevelDefault(undefined); // not supported by the omission-only proof.
  void topLevelDefault(props.value); // unknown activation, never guessed.
  const args: [number?] = [];
  void topLevelDefault(...args); // spread refuses the default collector.
  return <main />;
}

export function WrappedExactCall() {
  void (prefix as typeof prefix)(); // transparent wrapper, same exact target.
  return <main />;
}

export function ShadowedName() {
  async function prefix() { await Promise.resolve(); return 1; }
  void prefix(); // a different symbol, no count read.
  return <main />;
}

export function CrossFilePrefix() {
  void remotePrefix(); // violation, origin in remote.ts.
  return <main />;
}

export function CrossFileSuffix() {
  void remoteSuffix(); // uncertifiable, origin still in remote.ts.
  return <main />;
}
