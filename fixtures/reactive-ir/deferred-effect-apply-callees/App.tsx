import { createEffect, createMemo, createSignal, flush } from "solid-js";
import type { EffectOptions } from "@solidjs/signals";

// Clean: called only from applies that `defer: true` skips and no compute
// source can schedule, directly or through a closure, an IIFE or a helper.
function wrapDead() { createEffect(() => 1, () => {}); }
function wrapNested() { wrapInner(); }
function wrapInner() { createEffect(() => 1, () => {}); }
function wrapRecursive(depth: number) { if (depth > 0) wrapRecursive(depth - 1); createEffect(() => 1, () => {}); }
function wrapSyncDead() { createMemo(async () => 1, { sync: true }); }
function wrapFlushDead() { flush(); }
function helper(callback: () => unknown) { callback(); return 1; }

// Violation: a compute reading a retained writable source schedules the apply.
function wrapLive() { createEffect(() => 1, () => {}); }
function wrapSyncLive() { createMemo(async () => 1, { sync: true }); }
function wrapFlushLive() { flush(); }
// Violation: one live caller is enough, whatever other callers are dead.
function wrapMixed() { createEffect(() => 1, () => {}); }
// Uncertifiable: the defer value is unknown.
function wrapUnknown() { createEffect(() => 1, () => {}); }
function wrapSyncUnknown() { createMemo(async () => 1, { sync: true }); }
function wrapFlushUnknown() { flush(); }

export function App(props: { options: EffectOptions }) {
  const [count, setCount] = createSignal(0);
  createEffect(() => 1, () => { wrapDead(); }, { defer: true });
  createEffect(() => 1, () => { (() => { wrapNested(); })(); }, { defer: true });
  createEffect(() => 1, () => { helper(() => { wrapRecursive(1); }); }, { defer: true });
  createEffect(() => 1, () => { wrapSyncDead(); }, { defer: true });
  createEffect(() => 1, () => { wrapFlushDead(); }, { defer: true });
  createEffect(() => 1, () => { wrapMixed(); }, { defer: true });

  createEffect(() => count(), () => { wrapLive(); }, { defer: true });
  createEffect(() => count(), () => { wrapSyncLive(); }, { defer: true });
  createEffect(() => count(), () => { wrapFlushLive(); }, { defer: true });
  createEffect(() => 1, () => { wrapMixed(); });
  createEffect(() => 1, () => { wrapUnknown(); wrapSyncUnknown(); wrapFlushUnknown(); }, props.options);
  void setCount;
  return null;
}
