import { createEffect, createTrackedEffect, onCleanup } from "solid-js";
import * as S from "solid-js";

// ADR 1052: a check Solid makes earlier at the same call throws first, so the later rule's code is not proven.

// An effect created in an effect's apply: PRIMITIVE_IN_EFFECT_CALLBACK (SC6002) throws at the creation, before
// createEffect reports NO_OWNER_EFFECT: missing-owner is uncertifiable `creation-throws-first`.
export function EffectInApply() {
  createEffect(
    () => 1,
    () => {
      createEffect(() => 1, () => {});
    },
  );
  return <i />;
}
// The namespace form of the same.
export function NamespaceEffectInApply() {
  S.createEffect(
    () => 1,
    () => {
      S.createEffect(() => 1, () => {});
    },
  );
  return <i />;
}

// A helper a tracked effect and a handler both call: the tracked effect's call throws PRIMITIVE_IN_FORBIDDEN_SCOPE
// (SC3001) at mount and halts the reactive system, so the handler's unowned run of the same call never reports.
export function TrackedAndHandler() {
  const g = () => {
    createEffect(() => 1, () => {});
  };
  createTrackedEffect(() => {
    g();
  });
  return <button onClick={() => g()} />;
}
// The same with a cleanup: CLEANUP_IN_FORBIDDEN_SCOPE first.
export function TrackedAndHandlerCleanup() {
  const g = () => {
    onCleanup(() => {});
  };
  createTrackedEffect(() => {
    g();
  });
  return <button onClick={() => g()} />;
}

// An effect with no callable effect function in a tracked effect: MISSING_EFFECT_FN (SC7001) throws before the leaf
// owner's refusal: leaf-owner-forbidden-call is uncertifiable `creation-throws-first`.
export function MissingEffectFunctionInTracked() {
  const g = () => {
    createEffect(() => 1, undefined as unknown as () => void);
  };
  createTrackedEffect(() => {
    g();
  });
  return <i />;
}

// Part 3: a helper an effect's apply and a handler (or a render) both call. SC6002 is an obligation at the creation
// (`effect-owner-unproven`: one caller is an apply), and the apply's run throws PRIMITIVE_IN_EFFECT_CALLBACK and halts;
// missing-owner is uncertifiable `creation-throws-first`.
export function ApplyAndHandler() {
  const g = () => {
    createEffect(() => 1, () => {});
  };
  createEffect(
    () => 1,
    () => {
      g();
    },
  );
  return <button onClick={() => g()} />;
}
export function ApplyAndRender() {
  const g = () => {
    createEffect(() => 1, () => {});
  };
  createEffect(
    () => 1,
    () => {
      g();
    },
  );
  return <i>{(g(), "x")}</i>;
}

// Part 7: a module object's slot that only an effect's apply calls (`slotted.go()`): every unowned run of the effect it
// creates is on the apply's stack, where PRIMITIVE_IN_EFFECT_CALLBACK throws first: missing-owner is uncertifiable.
const slotted = {
  go: () => {
    createEffect(() => 1, () => {});
  },
};
export function SlotCalledInApply() {
  createEffect(
    () => 1,
    () => {
      slotted.go();
    },
  );
  return <i />;
}

// Violations kept: no earlier check throws at the call.
// A slot a timer calls too: the timer's run has no owner and reports NO_OWNER_EFFECT (a halt does not stop it).
const timed = {
  go: () => {
    createEffect(() => 1, () => {});
  },
};
export function SlotAlsoInTimer() {
  createEffect(
    () => 1,
    () => {
      timed.go();
    },
  );
  setTimeout(() => timed.go(), 0);
  return <i />;
}
export function HandlerOnly() {
  const g = () => {
    createEffect(() => 1, () => {});
  };
  return <button onClick={() => g()} />;
}
export function EffectFunctionPresentInTracked() {
  const g = () => {
    createEffect(() => 1, () => {});
  };
  createTrackedEffect(() => {
    g();
  });
  return <i />;
}
