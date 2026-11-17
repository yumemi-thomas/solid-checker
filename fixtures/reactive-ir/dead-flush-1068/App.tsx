import { createEffect, flush } from "solid-js";
import * as S from "solid-js";

// ADR 1068: FLUSH_IN_EFFECT_CALLBACK (SC2008) at a flush that provably never runs is uncertifiable (ADR 1018), not a violation.

// After a `return`: dead code.
export function AfterReturn() {
  createEffect(
    () => 1,
    () => {
      return;
      flush();
    },
  );
  return <i />;
}
// After a `try` that always returns, whose `finally` completes normally.
export function TryReturn() {
  createEffect(
    () => 1,
    () => {
      try {
        return;
      } finally {
        void 0;
      }
      flush();
    },
  );
  return <i />;
}
// In the apply of an effect whose compute throws first: the apply is never entered.
export function ThrowingCompute() {
  createEffect(
    (): number => {
      throw new Error("x");
    },
    () => {
      flush();
    },
  );
  return <i />;
}
// The namespace form.
export function NamespaceThrowingCompute() {
  S.createEffect(
    (): number => {
      throw new Error("x");
    },
    () => {
      S.flush();
    },
  );
  return <i />;
}
// In a callback of `forEach` on an empty array literal (ADR 1019's empty collection).
export function EmptyLiteral() {
  createEffect(
    () => 1,
    () => {
      ([] as number[]).forEach(() => flush());
    },
  );
  return <i />;
}

// Violations kept: the flush may run.
export function Plain() {
  createEffect(
    () => 1,
    () => {
      flush();
    },
  );
  return <i />;
}
export function CatchCompletes() {
  createEffect(
    () => 1,
    () => {
      try {
        throw new Error("x");
      } catch {
        void 0;
      }
      flush();
    },
  );
  return <i />;
}
export function ComputeMayReturn(props: { on: boolean }) {
  createEffect(
    (): number => {
      if (props.on) throw new Error("x");
      return 1;
    },
    () => {
      flush();
    },
  );
  return <i />;
}
