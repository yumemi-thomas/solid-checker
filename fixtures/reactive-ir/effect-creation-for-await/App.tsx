import { createEffect, createMemo } from "solid-js";
import { render } from "@solidjs/web";

// ADR 1215: a creation every run of which follows a suspension of its own async function runs on that continuation, a microtask on an
// empty stack, never inside the effect apply that started the function (rc.21 sets the effect-callback flag for the apply's synchronous
// run only). The suspensions: the `for await` around the creation (each pass awaits the iterator's `next()` first) and an `await` that
// dominates the rest of its block. Each apply below runs an async arrow it writes in place.

// Silent: `effect-creation-after-await`.
function ForAwaitBody() {
  createEffect(
    () => 0,
    () => {
      void (async () => {
        for await (const x of [1]) {
          createMemo(() => x);
        }
      })();
    },
  );
  return <p>a</p>;
}
function BranchBlockAwait() {
  createEffect(
    () => true,
    (on) => {
      void (async () => {
        if (on) {
          await 0;
          createMemo(() => 2);
        }
      })();
    },
  );
  return <p>b</p>;
}
function FinallyOwnAwait() {
  createEffect(
    () => 0,
    () => {
      void (async () => {
        try {
          void 0;
        } finally {
          await 0;
          createMemo(() => 3);
        }
      })();
    },
  );
  return <p>f</p>;
}

// Not this proof. A synchronous `for…of` runs its body on the apply's stack (PRIMITIVE_IN_EFFECT_CALLBACK), and so does a `for await`'s
// operand, evaluated before the first suspension: both keep SC6002's claim.
function ForOfSync() {
  createEffect(
    () => 0,
    () => {
      void (async () => {
        for (const x of [1]) {
          createMemo(() => x);
        }
      })();
    },
  );
  return <p>s</p>;
}
function ForAwaitOperand() {
  createEffect(
    () => 0,
    () => {
      void (async () => {
        for await (const x of [createMemo(() => 5)]) {
          void x;
        }
      })();
    },
  );
  return <p>o</p>;
}
// Uncertifiable, as before: a `finally` after a `try` whose `await` a synchronous throw of its operand would skip, and an async generator's
// body (its steps are the caller's).
function FinallyAfterTry() {
  createEffect(
    () => 0,
    () => {
      void (async () => {
        try {
          await Promise.resolve();
        } finally {
          createMemo(() => 6);
        }
      })();
    },
  );
  return <p>t</p>;
}
function AsyncGeneratorBody() {
  createEffect(
    () => 0,
    () => {
      void (async function* () {
        for await (const x of [1]) {
          createMemo(() => x);
          yield x;
        }
      })().next();
    },
  );
  return <p>g</p>;
}

render(
  () => (
    <p>
      <ForAwaitBody />
      <BranchBlockAwait />
      <FinallyOwnAwait />
      <ForOfSync />
      <ForAwaitOperand />
      <FinallyAfterTry />
      <AsyncGeneratorBody />
    </p>
  ),
  document.body,
);
