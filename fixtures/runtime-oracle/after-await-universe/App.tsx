import { Loading, createMemo, createSignal } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B (ADR 0973): a call written after an await keeps SC1002's uncertifiable row whatever its callee does (there is no callee reason: the review of #233's B1).
// A run no call shows (a getter, a coercion, an iterator, a Proxy trap) is not a call a closure follows, and the read written in such code has a row of its own where a
// computation is recognized (ADR 0963's member-route net), which these cases pin. Each export is the continuation of a tracked async computation calling a project function
// whose body reaches the read through a property access, a template literal, a spread or a Proxy: Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT at the read in that code, and the
// checker must have a row there.

const [viaGetter] = createSignal(1);
const holder = {
  get value() {
    return viaGetter();
  },
};
function peekGetter() {
  return holder.value;
}
export function ByGetter() {
  const data = createMemo(async () => {
    await pause();
    return peekGetter();
  });
  return (
    <Loading fallback="loading">
      <span>{data()}</span>
    </Loading>
  );
}

const [viaCoercion] = createSignal(2);
const labelled = {
  toString() {
    return String(viaCoercion());
  },
};
function labelOf() {
  return `${labelled}`;
}
export function ByCoercion() {
  const data = createMemo(async () => {
    await pause();
    return labelOf();
  });
  return (
    <Loading fallback="loading">
      <span>{data()}</span>
    </Loading>
  );
}

const [viaIterator] = createSignal(3);
const bag = {
  *[Symbol.iterator]() {
    yield viaIterator();
  },
};
function spreadBag() {
  return [...bag].length;
}
export function ByIterator() {
  const data = createMemo(async () => {
    await pause();
    return spreadBag();
  });
  return (
    <Loading fallback="loading">
      <span>{data()}</span>
    </Loading>
  );
}

const [viaTrap] = createSignal(4);
const trapped = new Proxy({} as { x?: number }, {
  get() {
    return viaTrap();
  },
});
function peekTrap() {
  return trapped.x;
}
export function ByProxy() {
  const data = createMemo(async () => {
    await pause();
    return peekTrap();
  });
  return (
    <Loading fallback="loading">
      <span>{data()}</span>
    </Loading>
  );
}

const [viaClass] = createSignal(5);
class Box {
  get value() {
    return viaClass();
  }
}
const instance = new Box();
function peekBox() {
  return instance.value;
}
export function ByClassGetter() {
  const data = createMemo(async () => {
    await pause();
    return peekBox();
  });
  return (
    <Loading fallback="loading">
      <span>{data()}</span>
    </Loading>
  );
}

// Control: the closure really reads nothing, so the continuation reads no reactive source and Chrome is quiet.
function double(n: number) {
  return n * 2;
}
function quad(n: number) {
  return double(double(n));
}
export function PureCallee() {
  const data = createMemo(async () => {
    await pause();
    return quad(2);
  });
  return (
    <Loading fallback="loading">
      <span>{data()}</span>
    </Loading>
  );
}
