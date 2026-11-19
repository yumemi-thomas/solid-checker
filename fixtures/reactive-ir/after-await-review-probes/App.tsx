import { createMemo, createSignal } from "solid-js";

// ADR 1097, the two reviews of its widening: an argument or a callee body that runs project code with no call. Each probe below runs in an async
// memo's continuation, so a signal read the project code makes is the memo's untracked read (rc.21: UNTRACKED_READ_AFTER_AWAIT). Every call that
// runs one keeps its SC1002 row, in the direct, wrapper and IIFE forms.

const pause = () => Promise.resolve();

export function Probes() {
  const [count] = createSignal(0);
  const [, setW] = createSignal<unknown>(0);
  const px = new Proxy({}, { has: () => count() > 0 });
  class K {
    static [Symbol.hasInstance](_value: unknown) {
      return count() > 0;
    }
  }
  const host = { toString: () => String(count()) };
  const key = host as unknown as string;

  // F1, `in`: a Proxy's `has` trap.
  const inWrapper = () => {
    setW("k" in px);
  };
  // F1, `instanceof`: `Symbol.hasInstance`.
  const instanceofWrapper = () => {
    setW(host instanceof K);
  };
  // F1, a computed key coerces its value.
  const keyWrapper = () => {
    setW({ [key]: 1 });
  };
  const all = createMemo(async () => {
    await pause();
    setW("k" in px);
    setW(host instanceof K);
    setW({ [key]: 1 });
    inWrapper();
    instanceofWrapper();
    keyWrapper();
    (() => {
      setW("k" in px);
    })();
    (() => {
      setW(host instanceof K);
    })();
    (() => {
      setW({ [key]: 1 });
    })();
    return 1;
  });
  return <p>{all()}</p>;
}

// F2: a callee body that probes `this` (a Proxy here) with `in`, `for…in` or `instanceof`.
export class ThisProbes {
  run(setW: (value: number) => void) {
    const inThis = () => {
      if ("x" in this) setW(1);
    };
    const forInThis = () => {
      for (const name in this) setW(name.length);
    };
    const instanceofThis = () => {
      if (this instanceof ThisProbes) setW(1);
    };
    return createMemo(async () => {
      await pause();
      inThis();
      forInThis();
      instanceofThis();
      (() => {
        if ("x" in this) setW(1);
      })();
      return 1;
    });
  }
}
