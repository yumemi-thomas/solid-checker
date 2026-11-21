import * as S from "solid-js";
import { createEffect, createReaction, createSignal, createStore, type EffectBundle } from "solid-js";

// ADR 1163: rc.21's `runEffect` runs a user effect's `error` member before it opens the effect's strict-read window, from the
// queue's drain. No `flush()` here drains on a stack with a window open, so a read written directly in the member reads in none.

// Clean: the read in the error arm of an effect, of a reaction, through the namespace, and a store member.
function EffectErrorRead() {
  const [count] = createSignal(3);
  const [trigger, setTrigger] = createSignal(1);
  createEffect(() => { if (trigger() === 2) throw new Error("x"); return 1; }, { effect: () => {}, error: () => {
    count();
  } });
  return <button onClick={() => setTrigger(2)}>go</button>;
}

function ReactionErrorRead() {
  const [count] = createSignal(3);
  const [trigger, setTrigger] = createSignal(1);
  const track = createReaction({ effect: () => {}, error: () => {
    count();
  } });
  track(() => { if (trigger() === 2) throw new Error("x"); });
  return <button onClick={() => setTrigger(2)}>go</button>;
}

function NamespaceErrorRead() {
  const [count] = S.createSignal(3);
  const [trigger, setTrigger] = S.createSignal(1);
  S.createEffect(() => { if (trigger() === 2) throw new Error("x"); return 1; }, { effect: () => {}, error: () => {
    count();
  } });
  return <button onClick={() => setTrigger(2)}>go</button>;
}

function StoreErrorRead() {
  const [store] = createStore({ v: 0 });
  const [trigger, setTrigger] = createSignal(1);
  createEffect(() => { if (trigger() === 2) throw new Error("x"); return 1; }, { effect: () => {}, error: () => {
    void store.v;
  } });
  return <button onClick={() => setTrigger(2)}>go</button>;
}

// Uncertifiable, as before: a bundle that is no literal, a spread bundle, and a closure the error arm hands on.
function NamedBundle() {
  const [count] = createSignal(3);
  const [trigger, setTrigger] = createSignal(1);
  const bundle: EffectBundle<number> = { effect: () => {}, error: () => {
    count();
  } };
  createEffect(() => { if (trigger() === 2) throw new Error("x"); return 1; }, bundle);
  return <button onClick={() => setTrigger(2)}>go</button>;
}

function SpreadBundle() {
  const [count] = createSignal(3);
  const [trigger, setTrigger] = createSignal(1);
  const base = { effect: () => {} };
  createEffect(() => { if (trigger() === 2) throw new Error("x"); return 1; }, { ...base, error: () => {
    count();
  } });
  return <button onClick={() => setTrigger(2)}>go</button>;
}

export function Page() {
  return (
    <div>
      <EffectErrorRead />
      <ReactionErrorRead />
      <NamespaceErrorRead />
      <StoreErrorRead />
      <NamedBundle />
      <SpreadBundle />
    </div>
  );
}
