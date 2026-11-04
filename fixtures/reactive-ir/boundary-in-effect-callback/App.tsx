import { Errored, Loading, createEffect, createReaction, createSignal, createTrackedEffect, getOwner, onSettled, resolve, runWithOwner, until } from "solid-js";

declare function log(): void;

// A boundary called as a function creates its computation in the call.
export function InReaction() {
  const [t, setT] = createSignal(1);
  const track = createReaction(() => {
    Loading({ fallback: 0, children: 1 }); // SC4001 and SC6002
  });
  track(() => {
    t();
  });
  return <button onClick={() => setT(2)}>go</button>;
}

export function ErroredInReaction() {
  const [t, setT] = createSignal(1);
  const track = createReaction(() => {
    Errored({ fallback: 0, children: 1 }); // SC4001 and SC6002
  });
  track(() => {
    t();
  });
  return <button onClick={() => setT(2)}>go</button>;
}

export function InApply() {
  createEffect(() => 1, () => {
    Loading({ fallback: 0, children: 1 }); // SC4001 and SC6002
  });
  return <p>x</p>;
}

export function Owned() {
  const owner = getOwner();
  createEffect(() => 1, () => {
    runWithOwner(owner, () => {
      Loading({ fallback: 0, children: 1 }); // clean: an owner is set
    });
  });
  return <p>x</p>;
}

export function InBody() {
  Loading({ fallback: 0, children: 1 }); // clean: the body owns it
  return <p>x</p>;
}

export function Shadowed() {
  createEffect(() => 1, () => {
    const Loading = (_: object) => 0;
    Loading({ fallback: 0, children: 1 }); // clean: a local function
  });
  return <p>x</p>;
}

export function EarlierCall() {
  createEffect(() => 1, () => {
    log();
    Loading({ fallback: 0, children: 1 }); // SC6002 uncertifiable: `log` may lower the marker
  });
  return <p>x</p>;
}

// `resolve` and `until` create a root and a computation for their thunk.
export function ResolveInApply() {
  createEffect(() => 1, () => {
    resolve(() => 1); // SC6002
  });
  return <p>x</p>;
}

export function UntilInReaction() {
  const [t, setT] = createSignal(1);
  const track = createReaction(() => {
    until(() => true); // SC6002
  });
  track(() => {
    t();
  });
  return <button onClick={() => setT(2)}>go</button>;
}

export function ResolveInHandler() {
  return <button onClick={() => { resolve(() => 1); }}>go</button>; // clean
}

// A leaf owner refuses what they create.
export function LeafSettled() {
  onSettled(() => {
    resolve(() => 1); // SC3001
  });
  return <p>x</p>;
}

export function LeafTracked() {
  createTrackedEffect(() => {
    Loading({ fallback: 0, children: 1 }); // SC3001
  });
  return <p>x</p>;
}
