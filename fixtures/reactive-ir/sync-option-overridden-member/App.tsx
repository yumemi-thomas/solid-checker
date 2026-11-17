import * as S from "solid-js";
import { createEffect, onSettled } from "solid-js";
import { h1 } from "./helpers";

// ADR 1034: `createEffect(async compute, apply, { sync: true })` is SC7002 (SYNC_NODE_RECEIVED_ASYNC) wherever it runs. Written in a
// function that is an object literal's member value a later spread, computed key or repeated key may replace, nothing proves that
// the function ever runs: uncertifiable `member-value-overridden`, never a violation.

// Uncertifiable: the member may be replaced (Chrome: the original never runs, no SYNC_NODE_RECEIVED_ASYNC).
export function SpreadAfter() {
  const extra: { run?: () => void } = { run: () => {} };
  const bundle = { run: () => {
    createEffect(async () => 1, () => {}, { sync: true });
  }, ...extra };
  bundle.run();
  return <p />;
}

export function ComputedAfter() {
  const key: string = "run";
  const bundle = { run: () => {
    createEffect(async () => 1, () => {}, { sync: true });
  }, [key]: () => {} };
  bundle.run();
  return <p />;
}

// A method, and a site in a closure the member calls.
export function MethodSpreadAfter() {
  const extra: { run?: () => void } = { run: () => {} };
  const bundle = { run() {
    const inner = () => { createEffect(async () => 1, () => {}, { sync: true }); };
    inner();
  }, ...extra };
  bundle.run();
  return <p />;
}

// Through a namespace import and a module helper's callback.
export function NamespaceHelper() {
  const key: string = "run";
  const bundle = { run: () => {
    h1(() => { S.createEffect(async () => 1, () => {}, { sync: true }); });
  }, [key]: () => {} };
  bundle.run();
  return <p />;
}

// The spread may not hold the key at all: Chrome runs the original and reports. The member is still not proven to hold it.
export function SpreadWithoutKey() {
  const extra: { run?: () => void } = {};
  const bundle = { run: () => {
    createEffect(async () => 1, () => {}, { sync: true });
  }, ...extra };
  bundle.run();
  return <p />;
}

// Violations: the member's final value (the spread or computed key comes first), and a plain member.
export function SpreadBefore() {
  const extra: { run?: () => void } = { run: () => {} };
  const bundle = { ...extra, run: () => {
    createEffect(async () => 1, () => {}, { sync: true });
  } };
  bundle.run();
  return <p />;
}

export function ComputedBefore() {
  const key: string = "other";
  const bundle = { [key]: () => {}, run: () => {
    createEffect(async () => 1, () => {}, { sync: true });
  } };
  bundle.run();
  return <p />;
}

export function Plain() {
  const bundle = { run: () => {
    createEffect(async () => 1, () => {}, { sync: true });
  }, label: "x" };
  bundle.run();
  return <p />;
}

// Shadowed: a later property of another object literal nested inside does not replace this one's member.
export function NestedLiteralLater() {
  const bundle = { run: () => {
    createEffect(async () => 1, () => {}, { sync: true });
  }, nested: { run: () => {} } };
  bundle.run();
  return <p />;
}

// The fuzz shape (ADR 1009's `dead-bundle-spread`): handed to `onSettled`. The replaced original never runs; Chrome is silent.
export function SettledSpreadAfter() {
  const extra: { run?: () => void } = { run: () => {} };
  const bundle = { run: () => {
    createEffect(async () => 1, () => {}, { sync: true });
  }, ...extra };
  onSettled(bundle.run);
  return <p />;
}
