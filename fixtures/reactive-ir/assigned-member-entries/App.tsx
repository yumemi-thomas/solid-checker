import { action, createEffect, createMemo, createSignal, createTrackedEffect, flush } from "solid-js";

const [n] = createSignal(0);
declare const toggled: boolean;

// ADR 1207: the program steps an action, runs an effect's apply and opens a leaf scope, so no program-wide reason holds and each flush stands on where
// it is placed. The flush is in a function a local `const` receiver's member is assigned once; the write dominates every call of the member, so the
// function's runs are those calls, asked as its entries.
export const step = action(function* () {
  yield 1;
});

export function Host() {
  createEffect(() => n(), () => {});
  createTrackedEffect(() => {
    n();
  });
  return <div />;
}

// Clean: write and optional call in a counted loop's later pass of an async memo (ADR 1201's class at the call).
export function AssignedLoop() {
  const m1 = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        const ho: { m?: () => void } = {};
        ho.m = () => {
          flush();
        };
        ho.m?.();
      }
      await Promise.resolve();
    }
    return 1;
  });
  void m1;
  return <div />;
}

// Clean: after an await of an async effect compute, the call in a branch the write's arms hold.
export function AssignedAfterAwait() {
  createEffect(
    async () => {
      await Promise.resolve();
      const ho: { m?: () => void } = {};
      ho.m = () => {
        flush();
      };
      if (toggled) {
        ho.m();
      }
    },
    () => {},
  );
  return <div />;
}

// Uncertifiable: two writes of the key.
export function TwoWrites() {
  createEffect(
    async () => {
      await Promise.resolve();
      const ho: { m?: () => void } = {};
      ho.m = () => {
        flush();
      };
      ho.m = () => {
        flush();
      };
      ho.m();
    },
    () => {},
  );
  return <div />;
}

// Uncertifiable: the write is in a branch the call is not in.
export function ConditionalWrite() {
  createEffect(
    async () => {
      await Promise.resolve();
      const ho: { m?: () => void } = {};
      if (toggled) {
        ho.m = () => {
          flush();
        };
      }
      ho.m?.();
    },
    () => {},
  );
  return <div />;
}

// Uncertifiable: the member is called in a closure (a timer's callback).
export function CapturedCall() {
  const ho: { m?: () => void } = {};
  ho.m = () => {
    flush();
  };
  setTimeout(() => {
    ho.m?.();
  }, 0);
  return <div />;
}

// Uncertifiable: the receiver is handed to a function that writes the member (fuzz `handoff-assign-param`).
export function WrittenThroughParameter() {
  createEffect(
    async () => {
      await Promise.resolve();
      const fill = (o: { m?: () => void }) => {
        o.m = () => {
          flush();
        };
      };
      const ho: { m?: () => void } = {};
      fill(ho);
      ho.m?.();
    },
    () => {},
  );
  return <div />;
}

// Uncertifiable: the call runs in the memo's compute before any await, on the compute's stack.
export function BeforeAwait() {
  const m1 = createMemo(async () => {
    const ho: { m?: () => void } = {};
    ho.m = () => {
      flush();
    };
    ho.m();
    await Promise.resolve();
    return 1;
  });
  void m1;
  return <div />;
}
