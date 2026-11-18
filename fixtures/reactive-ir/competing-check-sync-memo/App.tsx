import { action, createEffect, createMemo, createRenderEffect, createTrackedEffect, onSettled } from "solid-js";
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
// Through a helper the tracked effect calls: every reference to the helper is that refused call (part 5), so every run is
// refused; Chrome reports PRIMITIVE_IN_FORBIDDEN_SCOPE.
export function MemoInTrackedHelper() {
  const make = () => {
    createMemo(async () => 1, { sync: true });
  };
  createTrackedEffect(() => {
    make();
  });
  return <i />;
}

// Part 5: a helper only the tracked effect calls (every reference is that refused call).
export function MemoInTrackedOnlyHelper() {
  const make = () => {
    createMemo(async () => 1, { sync: true });
  };
  createTrackedEffect(() => {
    make();
  });
  createTrackedEffect(() => {
    make();
  });
  return <i />;
}
// Part 5: in a render effect's apply behind an early return: the owned first run may skip it, every later run is refused.
export function MemoInGuardedRenderApply(props: { on: number }) {
  createRenderEffect(
    () => props.on,
    (v) => {
      if (v === 1) return;
      createMemo(async () => 1, { sync: true });
    },
  );
  return <i />;
}

// Part 6: in the cleanup an effect's apply returns, when the effect may rerun: the rerun calls it inside the effect
// callback (PRIMITIVE_IN_EFFECT_CALLBACK first), and nothing orders that rerun against the disposal.
export function MemoInRerunCleanup(props: { n: number }) {
  createEffect(
    () => props.n,
    () => () => {
      createMemo(async () => 1, { sync: true });
    },
  );
  return <i />;
}
// Part 6: in the generator of an action only a leaf owner's callback calls: each call steps it under the leaf owner.
export function MemoInActionFromSettled() {
  const step = action(function* () {
    createMemo(async () => 1, { sync: true });
  });
  onSettled(() => {
    step();
  });
  return <i />;
}

// Violations kept: the render effect's owned first run creates it (straight line), and a helper the render also calls.
export function MemoInRenderApply() {
  createRenderEffect(
    () => 1,
    () => {
      createMemo(async () => 1, { sync: true });
    },
  );
  return <i />;
}
export function MemoInTrackedAndBodyHelper() {
  const make = () => {
    createMemo(async () => 1, { sync: true });
  };
  make();
  createTrackedEffect(() => {
    make();
  });
  return <i />;
}
// Violation kept: created in the component body, no earlier check throws.
// The effect never reruns (its compute tracks nothing): only the disposal calls the cleanup, outside the callback.
export function MemoInNeverRerunCleanup() {
  createEffect(
    () => 1,
    () => () => {
      createMemo(async () => 1, { sync: true });
    },
  );
  return <i />;
}
// An action an apply calls: the step lowers the effect flag and keeps the owner, so the probe runs.
export function MemoInActionFromApply() {
  const step = action(function* () {
    createMemo(async () => 1, { sync: true });
  });
  createEffect(
    () => 1,
    () => {
      step();
    },
  );
  return <i />;
}
export function MemoInBody() {
  createMemo(async () => 1, { sync: true });
  return <i />;
}
