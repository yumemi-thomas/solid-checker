import { createRoot, createSignal, getOwner, runWithOwner } from "solid-js";

const none: number[] = [];

function run(cb: () => unknown) {
  cb();
  return 1;
}

// Violations (SC1001), each confirmed by rc.17 in Chrome (STRICT_READ_UNTRACKED,
// probes/named-callback): a read in a layer a callback named at its sole
// `createRoot`/`runWithOwner` position runs in place (ADR 0517).
export function RootClosure() {
  const [s] = createSignal(0);
  const init = () => {
    const read = () => { s(); };
    read();
  };
  createRoot(init);
  return <div></div>;
}

export function RootDeclarationHelper() {
  const [s] = createSignal(0);
  function init() { run(() => { s(); }); }
  createRoot(init);
  return <div></div>;
}

export function RunWithOwnerClosure() {
  const [s] = createSignal(0);
  const owner = getOwner();
  const init = () => { const read = () => { s(); }; read(); };
  runWithOwner(owner, init);
  return <div></div>;
}

// Uncertifiable: two positions, a copy, a position in a handler.
export function RootShared() {
  const [s] = createSignal(0);
  const use = () => { s(); };
  const init = () => { use(); };
  createRoot(init);
  createRoot(init);
  return <div></div>;
}

export function RootCopy() {
  const [s] = createSignal(0);
  const init = () => { const read = () => { s(); }; read(); };
  const copy = init;
  createRoot(copy);
  return <div></div>;
}

export function RootInHandler() {
  const [s] = createSignal(0);
  const init = () => { const read = () => { s(); }; read(); };
  return <div><button onClick={() => createRoot(init)}>a</button></div>;
}

// Clean (rc.17 silent): JSX the callback returns, a timer, past an await.
export function RootJsx() {
  const [s] = createSignal(0);
  const init = () => <p>{s()}</p>;
  const view = createRoot(init);
  return <div>{view}</div>;
}

export function RootTimer() {
  const [s] = createSignal(0);
  const init = () => { setTimeout(() => { s(); }, 0); };
  createRoot(init);
  return <div></div>;
}

export function RootAfterAwait() {
  const [s] = createSignal(0);
  const init = async () => { await Promise.resolve(); const read = () => { s(); }; read(); };
  createRoot(init);
  return <div></div>;
}

// Review cases. Clean: JSX a layer returns. Uncertifiable: a library callback
// that may run zero times on the way, and a function named at its own
// `createRoot` (no placement, never a crash).
export function RootLayerJsx() {
  const [s] = createSignal(0);
  const init = () => { const view = () => <p>{s()}</p>; return view(); };
  const v = createRoot(init);
  return <div>{v}</div>;
}

export function RootEmptyForEach() {
  const [s] = createSignal(0);
  const init = () => { none.forEach(() => { s(); }); };
  createRoot(init);
  return <div></div>;
}

export function RootSelf() {
  const [s] = createSignal(0);
  function init() { createRoot(init); s(); }
  return <div>{s()}</div>;
}
