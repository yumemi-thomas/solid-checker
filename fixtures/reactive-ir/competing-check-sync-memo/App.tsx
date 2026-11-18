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
// Part 8 (1): a module object's slot only an effect's apply calls: the effect-callback guard first.
const slots = {
  go: () => {
    createMemo(async () => 1, { sync: true });
  },
};
export function MemoInSlotCalledInApply() {
  createEffect(
    () => 1,
    () => {
      slots.go();
    },
  );
  return <i />;
}
// Violation kept: a render effect's apply runs first at creation, owned, and probes.
const renderSlots = {
  go: () => {
    createMemo(async () => 1, { sync: true });
  },
};
export function MemoInSlotCalledInRenderApply() {
  createRenderEffect(
    () => 1,
    () => {
      renderSlots.go();
    },
  );
  return <i />;
}
// Part 8 (2): a prop callback an effect's apply calls, named, forwarded and spread: the effect-callback guard first.
function ApplyKid(props: { run: () => void }) {
  createEffect(
    () => 1,
    () => {
      props.run();
    },
  );
  return <i />;
}
function ApplyForward(props: { run: () => void }) {
  return <ApplyKid run={props.run} />;
}
function ApplySpread(props: { run: () => void }) {
  return <ApplyKid {...props} />;
}
export function MemoInNamedPropApply() {
  function run() {
    createMemo(async () => 1, { sync: true });
  }
  return <ApplyKid run={run} />;
}
export function MemoInForwardedPropApply() {
  return (
    <ApplyForward
      run={() => {
        createMemo(async () => 1, { sync: true });
      }}
    />
  );
}
export function MemoInSpreadPropApply() {
  return (
    <ApplySpread
      run={() => {
        createMemo(async () => 1, { sync: true });
      }}
    />
  );
}
// Violation kept: the forwarding component also calls the prop in its body, owned, and the probe runs.
function ApplyForwardAndCall(props: { run: () => void }) {
  props.run();
  return <ApplyKid run={props.run} />;
}
export function MemoInForwardedPropAlsoInBody() {
  return (
    <ApplyForwardAndCall
      run={() => {
        createMemo(async () => 1, { sync: true });
      }}
    />
  );
}
// Part 8 (3): a prop callback only an owned onSettled calls: the leaf owner's refusal first.
function SettledKid(props: { run: () => void }) {
  onSettled(() => {
    props.run();
  });
  return <i />;
}
function SettledForward(props: { run: () => void }) {
  return <SettledKid run={props.run} />;
}
export function MemoInPropSettled() {
  return (
    <SettledKid
      run={() => {
        createMemo(async () => 1, { sync: true });
      }}
    />
  );
}
export function MemoInForwardedPropSettled() {
  return (
    <SettledForward
      run={() => {
        createMemo(async () => 1, { sync: true });
      }}
    />
  );
}
// Violation kept: an onSettled called from a click handler has no owner, so its callback is no leaf owner's.
function HandlerSettledKid(props: { run: () => void }) {
  return (
    <button
      onClick={() => {
        onSettled(() => {
          props.run();
        });
      }}
    />
  );
}
export function MemoInPropUnownedSettled() {
  return (
    <HandlerSettledKid
      run={() => {
        createMemo(async () => 1, { sync: true });
      }}
    />
  );
}
// Part 9: a helper an effect's apply (or a tracked effect) and a handler of the same component call. The apply's run at
// mount throws first, and `render` disposes the root and its delegated events on that throw: the handler never runs.
export function MemoSharedApplyAndHandler() {
  const make = () => {
    createMemo(async () => 1, { sync: true });
  };
  createEffect(
    () => 1,
    () => {
      make();
    },
  );
  return <button onClick={() => make()} />;
}
export function MemoSharedTrackedAndHandler() {
  const make = () => {
    createMemo(async () => 1, { sync: true });
  };
  createTrackedEffect(() => {
    make();
  });
  return <button onClick={() => make()} />;
}
// Violation kept: a timer still runs the helper after the disposal, and the creation probes.
export function MemoSharedApplyAndTimer() {
  const make = () => {
    createMemo(async () => 1, { sync: true });
  };
  createEffect(
    () => 1,
    () => {
      make();
    },
  );
  setTimeout(() => make(), 0);
  return <i />;
}
// Violation kept: the effect is created only on one branch, so a mount may not throw and the handler runs.
export function MemoSharedConditionalApply(props: { on: boolean }) {
  const make = () => {
    createMemo(async () => 1, { sync: true });
  };
  if (props.on) {
    createEffect(
      () => 1,
      () => {
        make();
      },
    );
  }
  return <button onClick={() => make()} />;
}
