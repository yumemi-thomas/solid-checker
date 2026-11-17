import { action, createEffect } from "solid-js";

// ADR 1047: an action's step goes on at once, on the same stack, after a `yield` of a value that is not a thenable
// (`@solidjs/signals@2.0.0-rc.21` `dist/dev.js:2348-2365`). Each action is called from an effect's apply, which has no
// owner, so an effect created in a function the resumed step runs in place has none either: NO_OWNER_EFFECT (SC4001).

// Proved: an IIFE after `yield 1`.
export function IifeAfterNumber() {
  const a1 = action(function* () {
    yield 1;
    (() => {
      createEffect(() => 1, () => {});
    })();
  });
  createEffect(() => 1, () => {
    a1();
  });
  return <div />;
}

// Proved: a closure called after `yield "step"`.
export function ClosureAfterString() {
  const a2 = action(function* () {
    yield "step";
    const k2 = () => {
      createEffect(() => 2, () => {});
    };
    k2();
  });
  createEffect(() => 1, () => {
    a2();
  });
  return <div />;
}

// Proved: after a bare `yield` and after `yield null; yield true`.
export function AfterBareAndLiterals() {
  const a3 = action(function* () {
    yield;
    yield null;
    yield true;
    (() => {
      createEffect(() => 3, () => {});
    })();
  });
  createEffect(() => 1, () => {
    a3();
  });
  return <div />;
}

// Uncertifiable: a `yield` of a binding may hand out a thenable, after which the step runs from its continuation.
export function AfterBinding() {
  const p = Promise.resolve();
  const a4 = action(function* () {
    yield p;
    (() => {
      createEffect(() => 4, () => {});
    })();
  });
  createEffect(() => 1, () => {
    a4();
  });
  return <div />;
}

// Uncertifiable: a delegating `yield*` hands out whatever the delegate yields.
function* inner() {
  yield Promise.resolve();
}
export function AfterDelegate() {
  const a5 = action(function* () {
    yield* inner();
    (() => {
      createEffect(() => 5, () => {});
    })();
  });
  createEffect(() => 1, () => {
    a5();
  });
  return <div />;
}

// Uncertifiable: an async generator's `next()` returns a promise, so every `yield` moves the next step to a continuation.
export function AsyncGenerator() {
  const a6 = action(async function* () {
    yield 1;
    (() => {
      createEffect(() => 6, () => {});
    })();
  });
  createEffect(() => 1, () => {
    a6();
  });
  return <div />;
}

// Uncertifiable: `NaN` here is a local binding, not the number.
export function ShadowedName() {
  const NaN = Promise.resolve();
  const a7 = action(function* () {
    yield NaN;
    (() => {
      createEffect(() => 7, () => {});
    })();
  });
  createEffect(() => 1, () => {
    a7();
  });
  return <div />;
}

// Uncertifiable: the call after the `yield` may not run (it is under a branch), so its edge proves no run.
export function ConditionalAfterYield(props: { on: boolean }) {
  const a8 = action(function* () {
    yield 1;
    if (props.on) {
      (() => {
        createEffect(() => 8, () => {});
      })();
    }
  });
  createEffect(() => 1, () => {
    a8();
  });
  return <div />;
}

// Uncertifiable: a generator no action steps; whoever steps it decides when (and whether) its second step runs.
function* plain() {
  yield 1;
  (() => {
    createEffect(() => 9, () => {});
  })();
}
export function NotAnAction() {
  createEffect(() => 1, () => {
    const it = plain();
    it.next();
    it.next();
  });
  return <div />;
}
