import * as S from "solid-js";
import { createEffect, createOptimistic, createOptimisticStore, createRenderEffect } from "solid-js";

// ADR 1175: an optimistic setter in an effect apply's first run, before the async compute's first answer. Chrome throws
// OPTIMISTIC_WRITE_BEFORE_LOAD in Apply, RenderStore and Namespace, and is silent in Guarded, Declared and Reads.
export function Apply() {
  const [, setO] = createOptimistic(async () => 1);
  createEffect(
    () => 1,
    () => {
      setO(2);
    },
  );
  return <div />;
}

export function RenderStore() {
  const [, setO] = createOptimisticStore(async () => ({ v: 1 }), { v: 0 });
  createRenderEffect(
    () => 1,
    () => {
      setO((draft) => {
        draft.v = 2;
      });
    },
  );
  return <div />;
}

export function Namespace() {
  const [, setO] = S.createOptimistic(async () => 1);
  S.createEffect(
    () => 1,
    () => {
      setO(2);
    },
  );
  return <div />;
}

export function Guarded() {
  const [, setO] = createOptimistic(async () => 1);
  createEffect(
    () => 1,
    (v) => {
      if (v > 1) setO(2);
    },
  );
  return <div />;
}

export function Declared() {
  const [, setO] = createOptimistic(async () => 1, { loadingValue: 0 });
  createEffect(
    () => 1,
    () => {
      setO(2);
    },
  );
  return <div />;
}

export function Reads() {
  const [o, setO] = createOptimistic(async () => 1);
  createEffect(
    () => o(),
    () => {
      setO(2);
    },
  );
  return <div />;
}
