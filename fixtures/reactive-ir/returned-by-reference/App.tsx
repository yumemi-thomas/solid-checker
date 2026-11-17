import { createEffect, createMemo, untrack } from "solid-js";

// ADR 1062: a function an effect's or a memo's compute returns by reference or as a returned literal's member is a returned value
// (ADR 1020): a site it may run is SC7002 `creation-throws-first`, whatever other path also reaches it.

// R1: a named closure the compute returns by reference; the apply calls it.
export function R1() {
  createEffect(
    () => {
      const f = () => {
        createMemo(async () => 1, { sync: true });
      };
      return f;
    },
    (v) => {
      v();
    },
  );
  return <div />;
}

// R2: a closure returned as a literal's member; the apply calls the member.
export function R2() {
  createEffect(
    () => ({
      run: () => {
        createMemo(async () => 2, { sync: true });
      },
    }),
    (v) => {
      v.run();
    },
  );
  return <div />;
}

// R3: a named closure returned as a member by reference.
export function R3() {
  createEffect(
    () => {
      const f = () => {
        createMemo(async () => 3, { sync: true });
      };
      return { run: f };
    },
    (v) => {
      v.run();
    },
  );
  return <div />;
}

// R4 (second path): the same helper is also run by an exported function no module here calls, and by the returned closure the apply calls.
function makeSync() {
  createMemo(async () => 4, { sync: true });
}
export function R4() {
  createEffect(
    () => () => {
      makeSync();
    },
    (v) => {
      v();
    },
  );
  return <div />;
}
export function alsoRunsHelper() {
  makeSync();
}

// R5 (second path): a named returned closure also referenced by a never-called local.
export function R5() {
  createEffect(
    () => {
      const f = () => {
        createMemo(async () => 5, { sync: true });
      };
      const unused = () => f();
      void unused;
      return f;
    },
    (v) => {
      v();
    },
  );
  return <div />;
}

// Control C1: a memo's value is a returned-by-reference closure called in the body: SYNC_NODE (stays uncertifiable, fail closed).
export function C1() {
  const m = createMemo(() => {
    const f = () => {
      createMemo(async () => 6, { sync: true });
    };
    return f;
  });
  untrack(m)();
  return <div />;
}

// Control C2: a sync memo created directly in the body (no returned value anywhere on its path): violation.
export function C2() {
  createMemo(async () => 7, { sync: true });
  return <div />;
}
