import { createEffect, createMemo, createTrackedEffect, onSettled } from "solid-js";
import * as S from "solid-js";

// ADR 1052, part 2: `createMemo(async, { sync: true })` throws SYNC_NODE_RECEIVED_ASYNC (SC7002) only at its result probe; a
// check Solid makes earlier at the creation throws first, so SC7002 is uncertifiable `creation-throws-first` there.

// In an effect's apply: PRIMITIVE_IN_EFFECT_CALLBACK (SC6002) first.
export function MemoInApply() {
  createEffect(
    () => 1,
    () => {
      createMemo(async () => 1, { sync: true });
    },
  );
  return <i />;
}
// The namespace form.
export function NamespaceMemoInApply() {
  S.createEffect(
    () => 1,
    () => {
      S.createMemo(async () => 1, { sync: true });
    },
  );
  return <i />;
}
// In a tracked effect and in `onSettled`: the leaf owner's refusal, PRIMITIVE_IN_FORBIDDEN_SCOPE (SC3001), first.
export function MemoInTracked() {
  createTrackedEffect(() => {
    createMemo(async () => 1, { sync: true });
  });
  return <i />;
}
export function MemoInSettled() {
  onSettled(() => {
    createMemo(async () => 1, { sync: true });
  });
  return <i />;
}
// Kept (open): through a helper the tracked effect calls. A refusal reached through a helper refuses that caller's run
// only, and another caller (a render) may reach the probe first, so it is not counted; Chrome reports PRIMITIVE_IN_FORBIDDEN_SCOPE.
export function MemoInTrackedHelper() {
  const make = () => {
    createMemo(async () => 1, { sync: true });
  };
  createTrackedEffect(() => {
    make();
  });
  return <i />;
}

// Violation kept: created in the component body, no earlier check throws.
export function MemoInBody() {
  createMemo(async () => 1, { sync: true });
  return <i />;
}
