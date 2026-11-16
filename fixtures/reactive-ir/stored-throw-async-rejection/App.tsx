import { createContext, createMemo, createRenderEffect, untrack, useContext } from "solid-js";

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
