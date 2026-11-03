import { createContext, onCleanup, useContext } from "solid-js";

// Default-less and never provided: a read under an owner throws ContextNotFoundError.
const Ctx = createContext<number>();

// Positive (SC4002): `reduce` with two elements calls its callback once.
export function ReduceTwo() {
  const hf = (a: number, b: number) => {
    useContext(Ctx);
    return a + b;
  };
  [1, 2].reduce(hf);
  return <i>reduce-two</i>;
}

// Positive (SC4002): `reduce` with one element and an initial value calls it once.
export function ReduceInitial() {
  const hf = (a: number, b: number) => {
    useContext(Ctx);
    return a + b;
  };
  [1].reduce(hf, 0);
  return <i>reduce-initial</i>;
}

// Positive (SC4002), member path: `globalThis.Reflect` is the standard library's `Reflect`.
export function GlobalReflect() {
  const hf = () => {
    useContext(Ctx);
  };
  globalThis.Reflect.apply(hf, null, []);
  return <i>global-reflect</i>;
}

// Positive (SC4002): a non-empty literal behind a type assertion.
export function AssertedLiteral() {
  ([1] as number[]).forEach(() => useContext(Ctx));
  return <i>asserted</i>;
}

// Positive (SC4002): the `Promise` constructor runs its executor before it returns.
export function PromiseExecutor() {
  new Promise<void>(() => {
    useContext(Ctx);
  });
  return <i>executor</i>;
}

// Negative (SC4001 obligation, never SC4002): an empty receiver runs the callback zero times.
export function EmptyNamed() {
  const hf = () => {
    useContext(Ctx);
  };
  ([] as number[]).forEach(hf);
  return <i>empty-named</i>;
}

export function EmptyLiteral() {
  ([] as number[]).map(() => useContext(Ctx));
  return <i>empty-literal</i>;
}

// Negative (obligation): a receiver of unknown length may be empty.
export function PropsReceiver(props: { xs: number[] }) {
  const hf = () => {
    useContext(Ctx);
  };
  props.xs.forEach(hf);
  return <i>props</i>;
}

export function VariableReceiver() {
  const xs: number[] = [];
  const hf = () => {
    useContext(Ctx);
  };
  xs.forEach(hf);
  return <i>variable</i>;
}

// Negative (obligation): a spread may contribute no element.
export function SpreadReceiver(props: { xs: number[] }) {
  const hf = () => {
    useContext(Ctx);
  };
  [...props.xs].forEach(hf);
  return <i>spread</i>;
}

// Negative (obligation): a `Set`'s size is not read here.
export function SetForEach() {
  const hf = () => {
    useContext(Ctx);
  };
  new Set([1]).forEach(hf);
  return <i>set</i>;
}

// Negative (obligation): one element and no initial value: `reduce` returns it uncalled.
export function ReduceOne() {
  const hf = (a: number, b: number) => {
    useContext(Ctx);
    return a + b;
  };
  [1].reduce(hf);
  return <i>reduce-one</i>;
}

// Negative (obligation): `sort` compares nothing in one element, and orders `undefined` uncalled.
export function SortOne() {
  const hf = (_a: number, _b: number) => {
    useContext(Ctx);
    return 0;
  };
  [1].sort(hf);
  return <i>sort-one</i>;
}

export function SortUndefined() {
  const hf = (_a: number | undefined, _b: number | undefined) => {
    useContext(Ctx);
    return 0;
  };
  [undefined, 1].sort(hf);
  return <i>sort-undefined</i>;
}

// Negative (obligation): `Array.from` maps nothing from an empty source.
export function FromEmpty() {
  const hf = () => useContext(Ctx);
  Array.from([] as number[], hf);
  return <i>from-empty</i>;
}

// Negative (obligation): a local `Reflect` is not the standard library's.
export function LocalReflect() {
  const Reflect = { apply(_f: () => void, _t: null, _a: []) {} };
  const hf = () => {
    useContext(Ctx);
  };
  Reflect.apply(hf, null, []);
  return <i>local-reflect</i>;
}

// SC4001: in a timer, a non-empty carrier runs the callback with no owner (violation); an empty one
// may run it zero times (obligation).
export function TimerNonEmpty() {
  setTimeout(() => {
    [1].forEach(() => {
      onCleanup(() => {});
    });
  });
  return <i>timer-non-empty</i>;
}

export function TimerEmpty() {
  setTimeout(() => {
    ([] as number[]).forEach(() => {
      onCleanup(() => {});
    });
  });
  return <i>timer-empty</i>;
}
