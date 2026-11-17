import { action, createSignal, onFlush } from "solid-js";

const act = action(function* () {
  yield 1;
});

// ---- Never run: the carrier is the standard-library one and its receiver or source is a literal
// nothing can fill; the write, the action call and onFlush are U (unreached), never a violation ----

export function EmptyForEach() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(1);
  };
  ([] as number[]).forEach(tf);
  return <p />;
}
export function EmptyInlineLiteral() {
  const [, setW] = createSignal(0);
  ([] as number[]).map(() => {
    setW(2);
  });
  return <p />;
}
export function EmptyReadonlyFilter() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(3);
    return true;
  };
  ([] as readonly number[]).filter(tf);
  return <p />;
}
export function EmptySort() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(4);
    return 0;
  };
  ([] as number[]).sort(tf);
  return <p />;
}
// One element and no initial value: `reduce` returns the element uncalled.
export function ReduceOneNoInitial() {
  const [, setW] = createSignal(0);
  const tf = (a: number) => {
    setW(5);
    return a;
  };
  [1].reduce(tf);
  return <p />;
}
export function FromEmpty() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(6);
    return 0;
  };
  Array.from([], tf);
  Uint8Array.from([], tf);
  return <p />;
}
export function FreshSet() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(8);
  };
  new Set<number>().forEach(tf);
  return <p />;
}
export function FreshMapAction() {
  const [, setW] = createSignal(0);
  new Map<number, number>().forEach(() => {
    void act();
  });
  return <p />;
}
export function EmptyOnFlush() {
  const [, setW] = createSignal(0);
  ([] as number[]).forEach(() => {
    onFlush(() => {});
  });
  return <p />;
}
export function ReplaceNoMatch() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(9);
    return "y";
  };
  "x".replace("z", tf);
  "x".replaceAll("z", tf);
  return <p />;
}

// ---- May run: today's reading stays (a violation where the body is an owned scope) ----

export function NonEmpty() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(10);
  };
  [1].forEach(tf);
  return <p />;
}
// A name holds the empty array: what the name holds is not read.
export function NamedEmpty() {
  const [, setW] = createSignal(0);
  const xs: number[] = [];
  const tf = () => {
    setW(11);
  };
  xs.forEach(tf);
  return <p />;
}
export function ReduceWithInitial() {
  const [, setW] = createSignal(0);
  const tf = (a: number) => {
    setW(12);
    return a;
  };
  [1].reduce(tf, 0);
  return <p />;
}
export function SetWithElement() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(13);
  };
  new Set([1]).forEach(tf);
  return <p />;
}
export function ReplaceMatch() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(14);
    return "y";
  };
  "x".replace("x", tf);
  return <p />;
}
// The empty carrier is one reference; another reference calls the function.
export function AlsoCalled() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(15);
  };
  ([] as number[]).forEach(tf);
  tf();
  return <p />;
}
// A wrapper hands the array on: the carrier's receiver is a parameter.
function each(xs: number[], f: () => void) {
  xs.forEach(f);
}
export function Wrapped() {
  const [, setW] = createSignal(0);
  each([], () => {
    setW(16);
  });
  return <p />;
}
// A NodeList the DOM fills: not a literal.
export function ChildNodes() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(17);
  };
  document.createElement("i").childNodes.forEach(tf);
  return <p />;
}
