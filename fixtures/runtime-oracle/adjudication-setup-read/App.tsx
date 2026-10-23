// Corpus adjudication 2026-10-23 (scripts/corpus/reviewed-findings.json): reads
// in a component body that seed state once. rc.17 observedComponent runs the
// body under untrack(..., "<Name>") (solid-js dist/solid.dev.js:44-66), so each
// read warns STRICT_READ_UNTRACKED.
import { createSignal, createStore } from "solid-js";

// probus-hk Plan.tsx:254-256, Search.tsx:302: state seeded from a signal and
// through a helper that reads another.
export function SeedRead() {
  const [search] = createSignal({ from: "s1", to: "" });
  const [db] = createSignal<Record<string, string>>({ s1: "Central" });
  const endOf = (value: string) => (value ? db()[value] ?? null : null);
  const [from] = createSignal(endOf(search().from));
  const [to] = createSignal(endOf(search().to));
  const [picking] = createSignal(to() ? "none" : "to");
  return <p>{from()} {picking()}</p>;
}

// kui health.ts:86 via App.tsx:154: a factory the body calls reads the signal it
// has just created.
function createHealth() {
  const [connectivity] = createSignal({ kind: "connected" });
  const current = connectivity();
  return { connectivity, current };
}
export function FactoryRead() {
  const health = createHealth();
  return <p>{health.connectivity().kind} {health.current.kind}</p>;
}

// app-game rapier-2d-physics-engine/index.tsx:40-50: store paths read in the body.
export function StoreRead() {
  const [state] = createStore({ ground: { hx: 6, hy: 0.1 } });
  const desc = [state.ground.hx, state.ground.hy];
  return <p>{desc.join(",")}</p>;
}
