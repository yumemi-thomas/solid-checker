import { action, createEffect, createMemo, createSignal, createTrackedEffect, flush } from "solid-js";

const [n] = createSignal(0);

// ADR 1205: the program steps an action, runs an effect's apply and opens a leaf scope, so no program-wide reason holds and each flush stands on where
// it is placed. The flush is in a function a closed object literal holds under a key; its every run is a member call of that literal, which the member
// census resolves exactly, so the flush is asked at those calls.
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

// Clean: the member call is in a counted loop's later pass of an async memo (ADR 1201's class at the call).
export function MemberLoop() {
  const m1 = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        const hf = () => {
          flush();
        };
        const ho = { m: hf };
        ho.m();
      }
      await Promise.resolve();
    }
    return 1;
  });
  void m1;
  return <div />;
}

// Clean: the function literal is the slot's value, called after an await of an async effect compute.
export function LiteralSlot() {
  createEffect(
    async () => {
      await Promise.resolve();
      const ho = {
        m: () => {
          flush();
        },
      };
      ho.m();
    },
    () => {},
  );
  return <div />;
}

// Uncertifiable: the member call is written in a closure (a timer's callback), where the member census refuses the receiver: a captured
// binding has no entry-order proof (ADR 0299), so the call resolves to nothing and the literal's function has no exact entry list.
export function CapturedCall() {
  const ho = {
    m: () => {
      flush();
    },
  };
  setTimeout(() => {
    ho.m();
  }, 0);
  return <div />;
}

// Clean: an optional member call after an await in an async effect compute.
export function OptionalCall() {
  createEffect(
    async () => {
      await Promise.resolve();
      const hf = () => {
        flush();
      };
      const ho = { m: hf };
      ho.m?.();
    },
    () => {},
  );
  return <div />;
}

// Uncertifiable: the member call runs in the loop's first pass too (no `i > 0` guard), before any await.
export function FirstPass() {
  const m1 = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      const hf = () => {
        flush();
      };
      const ho = { m: hf };
      ho.m();
      await Promise.resolve();
    }
    return 1;
  });
  void m1;
  return <div />;
}

// Uncertifiable: the member is read without a call and the value called elsewhere, which no member call shows.
export function MemberRead() {
  const m1 = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      flush();
    };
    const ho = { m: hf };
    const g = ho.m;
    g();
    return 1;
  });
  void m1;
  return <div />;
}

// Uncertifiable: the object is handed to a callback, which may call the member anywhere.
export function ObjectHanded() {
  const m1 = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      flush();
    };
    const ho = { m: hf };
    [ho].forEach((o) => o.m());
    return 1;
  });
  void m1;
  return <div />;
}

// Uncertifiable: the binding is a `let`, which the program may write.
export function LetHolder() {
  const m1 = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      flush();
    };
    let ho = { m: hf };
    ho.m();
    ho = { m: () => {} };
    return 1;
  });
  void m1;
  return <div />;
}

// Uncertifiable: the member call runs in the memo's compute before any await, on the compute's stack.
export function BeforeAwait() {
  const m1 = createMemo(async () => {
    const hf = () => {
      flush();
    };
    const ho = { m: hf };
    ho.m();
    await Promise.resolve();
    return 1;
  });
  void m1;
  return <div />;
}
