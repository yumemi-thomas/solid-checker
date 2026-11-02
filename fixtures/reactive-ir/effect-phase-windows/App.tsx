import { createEffect, createMemo, createReaction, createRenderEffect, createSignal, flush, getOwner, runWithOwner } from "solid-js";

function run(cb: () => unknown) {
  cb();
}

// A reaction's callback runs only as its effect's apply, in the draining
// flush with no owner: a flush there is a no-op and a memo throws.
export function ReactionPhase() {
  const [tick, setTick] = createSignal(1);
  const track = createReaction(() => {
    flush();
  });
  track(() => tick());
  const created = createReaction(() => {
    createMemo(() => 1);
  });
  created(() => tick());
  const layered = createReaction(() => {
    run(() => createMemo(() => 2));
  });
  layered(() => tick());
  return <button onClick={() => setTick(2)}>go</button>;
}

// The callback named by a binding: the same.
export function ReactionNamed() {
  const [tick, setTick] = createSignal(1);
  const onChange = () => {
    flush();
  };
  const track = createReaction(onChange);
  track(() => tick());
  return <button onClick={() => setTick(2)}>go</button>;
}

// A render effect's apply runs first at creation (owned, no flush), later
// from the flush with no owner: obligations.
export function RenderReruns() {
  const [tick, setTick] = createSignal(1);
  createRenderEffect(
    () => tick(),
    () => {
      flush();
      createMemo(() => 1);
    },
  );
  return <button onClick={() => setTick(2)}>go</button>;
}

// The cleanup an apply returns runs on a rerun (in the phase) or on a
// disposal (not): obligations.
export function CleanupReruns() {
  const [tick, setTick] = createSignal(1);
  createEffect(
    () => tick(),
    () => () => {
      flush();
      createMemo(() => 1);
    },
  );
  return <button onClick={() => setTick(2)}>go</button>;
}

// Clean: an owner set inside the reaction; a reaction's tracking function
// (run at the `track` call, in the component body).
export function ReactionControls() {
  const [tick, setTick] = createSignal(1);
  const owner = getOwner();
  const track = createReaction(() => {
    runWithOwner(owner, () => createMemo(() => 1));
  });
  track(() => {
    tick();
  });
  return <button onClick={() => setTick(2)}>go</button>;
}

// A project function named like the primitive is no reaction.
function createReactionLike(fn: () => void) {
  return (tracking: () => void) => {
    tracking();
    queueMicrotask(fn);
  };
}

export function ShadowedReaction() {
  const [tick] = createSignal(1);
  const track = createReactionLike(() => {
    flush();
  });
  track(() => tick());
  return <button>go</button>;
}
