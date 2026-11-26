import { createEffect, createMemo } from "solid-js";

export function ForAwaitBody() {
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

export function BranchBlockAwait() {
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

export function FinallyOwnAwait() {
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

export function FinallyAfterTry() {
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

export function AsyncGeneratorBody() {
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
