import { createContext, createMemo, createRenderEffect, createTrackedEffect, flush, resolve, untrack, until, useContext } from "solid-js";

// A default: a read with no owner throws NoOwnerError.
const Ctx = createContext(7);

const boom = async () => {
  await Promise.resolve();
  useContext(Ctx);
};

// Violations: nothing holds the promise the throw rejects.
export function Discarded() {
  return <p ref={() => { (async () => { await Promise.resolve(); useContext(Ctx); })(); }}>x</p>;
}

export function Voided() {
  return <p ref={() => { void (async () => { await Promise.resolve(); useContext(Ctx); })(); }}>x</p>;
}

export function SyncTwin() {
  return <p ref={() => { (() => { useContext(Ctx); })(); }}>x</p>;
}

export function NamedDiscarded() {
  return <p ref={() => { const run = async () => { await Promise.resolve(); useContext(Ctx); }; run(); }}>x</p>;
}

export function BeforeAwaitDiscarded() {
  return <p ref={() => { (async () => { useContext(Ctx); })(); }}>x</p>;
}

export function ThenNoReject() {
  return <p ref={() => { (async () => { await Promise.resolve(); useContext(Ctx); })().then(() => {}); }}>x</p>;
}

export function AwaitedInDiscarded() {
  return <p ref={() => { (async () => { await (async () => { await Promise.resolve(); useContext(Ctx); })(); })(); }}>x</p>;
}

// Obligations: the promise is caught, held, returned, awaited under a `try` or handed on.
export function Caught() {
  return <p ref={() => { (async () => { await Promise.resolve(); useContext(Ctx); })().catch(() => {}); }}>x</p>;
}

export function BeforeAwaitCaught() {
  return <p ref={() => { (async () => { useContext(Ctx); })().catch(() => {}); }}>x</p>;
}

export function ThenRejectArm() {
  return <p ref={() => { (async () => { await Promise.resolve(); useContext(Ctx); })().then(() => {}, () => {}); }}>x</p>;
}

export function Held() {
  return <p ref={() => { const p = (async () => { await Promise.resolve(); useContext(Ctx); })(); void p.catch(() => {}); }}>x</p>;
}

export function NamedCaught() {
  return <p ref={() => { const run = async () => { await Promise.resolve(); useContext(Ctx); }; run().catch(() => {}); }}>x</p>;
}

export function AwaitedInCaught() {
  return <p ref={() => { (async () => { await (async () => { await Promise.resolve(); useContext(Ctx); })(); })().catch(() => {}); }}>x</p>;
}

export function AwaitedInTry() {
  return <p ref={() => { (async () => { try { await (async () => { await Promise.resolve(); useContext(Ctx); })(); } catch {} })(); }}>x</p>;
}

export function Returned() {
  const start = () => (async () => { await Promise.resolve(); useContext(Ctx); })();
  return <p ref={() => { start().catch(() => {}); }}>x</p>;
}

export function InnerSyncCaught() {
  return <p ref={() => { (async () => { await Promise.resolve(); (() => { useContext(Ctx); })(); })().catch(() => {}); }}>x</p>;
}

export function HelperCaught() {
  const run = async (callback: () => void) => { await Promise.resolve(); callback(); };
  return <p ref={() => { run(() => { useContext(Ctx); }).catch(() => {}); }}>x</p>;
}

export function ModuleCaught() {
  return <p ref={() => { boom().catch(() => {}); }}>x</p>;
}

export function UntrackCaught() {
  return <p ref={() => { untrack(async () => { await Promise.resolve(); useContext(Ctx); }).catch(() => {}); }}>x</p>;
}

export function UntrackHeld() {
  return <p ref={() => { const p = untrack(async () => { await Promise.resolve(); useContext(Ctx); }); void p.catch(() => {}); }}>x</p>;
}

export function MapCaught() {
  return <p ref={() => { Promise.all([1].map(async () => { await Promise.resolve(); useContext(Ctx); })).catch(() => {}); }}>x</p>;
}

// Violations again: a carrier, a module function and a primitive that drop the promise.
export function ForEachDiscarded() {
  return <p ref={() => { [1].forEach(async () => { await Promise.resolve(); useContext(Ctx); }); }}>x</p>;
}

export function ModuleDiscarded() {
  return <p ref={() => { boom(); }}>x</p>;
}

export function UntrackDiscarded() {
  return <p ref={() => { untrack(async () => { await Promise.resolve(); useContext(Ctx); }); }}>x</p>;
}

// Obligations: the same promise caught inside a compute that reports a plain throw.
export function EffectCaught() {
  createRenderEffect(() => { (async () => { await Promise.resolve(); useContext(Ctx); })().catch(() => {}); }, () => {});
  return <div></div>;
}

export function MemoCaught() {
  const m = createMemo(() => { (async () => { await Promise.resolve(); useContext(Ctx); })().catch(() => {}); return 1; });
  return <div>{m()}</div>;
}

// The review of ADR 1008. Violations: a handler's and a `ref`'s return value is dropped by the host, and a helper's promise by its call.
const fromHandler = async () => {
  await Promise.resolve();
  useContext(Ctx);
};

const fromRef = async () => {
  await Promise.resolve();
  useContext(Ctx);
};

export function HandlerReturned() {
  return <button onClick={() => fromHandler()}>x</button>;
}

export function RefReturned() {
  return <p ref={() => fromRef()}>x</p>;
}

export function HelperResolveDiscarded() {
  const run = async () => { void resolve(() => 1); };
  createTrackedEffect(() => { run(); });
  return <div></div>;
}

// Obligations: an `await` of what `catch` or a two-armed `then` returns, a named `async` function a primitive runs, and a
// helper's `resolve`, `until` or `flush` whose promise the tracked effect catches.
export function AwaitCatch() {
  return <p ref={() => { (async () => { await (async () => { await Promise.resolve(); useContext(Ctx); })().catch(() => {}); })(); }}>x</p>;
}

export function AwaitThenArms() {
  return <p ref={() => { (async () => { await (async () => { await Promise.resolve(); useContext(Ctx); })().then(() => {}, () => {}); })(); }}>x</p>;
}

export function UntrackNamedCaught() {
  const run = async () => { await Promise.resolve(); useContext(Ctx); };
  return <p ref={() => { untrack(run).catch(() => {}); }}>x</p>;
}

export function UntrackAliasCaught() {
  const run = async () => { await Promise.resolve(); useContext(Ctx); };
  const again = run;
  return <p ref={() => { untrack(again).catch(() => {}); }}>x</p>;
}

export function HelperResolveCaught() {
  const run = async () => { void resolve(() => 1); };
  createTrackedEffect(() => { run().catch(() => {}); });
  return <div></div>;
}

export function HelperUntilCaught() {
  const run = async () => { void until(() => 1); };
  createTrackedEffect(() => { run().catch(() => {}); });
  return <div></div>;
}

export function HelperFlushCaught() {
  const run = async () => { flush(); };
  createTrackedEffect(() => { run().catch(() => {}); });
  return <div></div>;
}

export function LiteralResolveCaught() {
  createTrackedEffect(() => { (async () => { void resolve(() => 1); })().catch(() => {}); });
  return <div></div>;
}

export function DeepHelperCaught() {
  const inner = async () => { void resolve(() => 1); };
  const run = () => { inner().catch(() => {}); };
  createTrackedEffect(() => { run(); });
  return <div></div>;
}
