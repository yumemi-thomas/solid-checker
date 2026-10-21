import * as solid from "solid-js";
declare const flag: boolean;
declare const pending: Promise<number>;

async function namespaceCleanup() {
  if (flag) await pending;
  solid.onCleanup(() => {});
}
export function NamespaceCleanup() {
  solid.onSettled(() => { void namespaceCleanup(); });
  return <div />;
}

// No invocation: syntax must not seed an execution for this private helper.
async function dormant() {
  if (flag) await pending;
  solid.onCleanup(() => {});
}

export function ShadowedCleanup() {
  function onCleanup(_callback: () => void) {}
  async function run() {
    if (flag) await pending;
    onCleanup(() => {});
  }
  solid.onSettled(() => { void run(); });
  return <div />;
}

function registerWrapped() { solid.onCleanup(() => {}); }
async function wrapped() {
  if (flag) await pending;
  registerWrapped();
}
export function WrappedCleanup() {
  solid.onSettled(() => { void wrapped(); });
  return <div />;
}
async function callback() {
  if (flag) await pending;
  solid.untrack(() => solid.onCleanup(() => {}));
}
export function CallbackCleanup() {
  solid.onSettled(() => { void callback(); });
  return <div />;
}

async function freshRoot() {
  if (flag) await pending;
  solid.createRoot(() => solid.onCleanup(() => {}));
}
export function FreshRootCleanup() {
  solid.onSettled(() => { void freshRoot(); });
  return <div />;
}
