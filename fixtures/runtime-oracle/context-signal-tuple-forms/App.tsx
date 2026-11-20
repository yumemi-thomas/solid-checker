import { createContext, createMemo, createSignal, useContext, type Accessor, type Setter } from "solid-js";

type Pair<T> = readonly [Accessor<T>, Setter<T>];

// ADR 1123, the review's F1: the tuple arm takes only a plain value signal. Each consumer reads its slot in the body, which a proof
// would make SC1001; the function form, an unproven value and an options object stay uncertifiable.

// Proved (SC1001): a literal value.
const PlainContext = createContext<Pair<string>>();
function Plain() {
  const [get] = useContext(PlainContext);
  const now = get();
  return <span>{now}</span>;
}
export function PlainApp() {
  const value = createSignal("p");
  return (
    <PlainContext value={value}>
      <Plain />
    </PlainContext>
  );
}

// Proved (SC1001): no argument, the value `undefined`.
const EmptyContext = createContext<Pair<string | undefined>>();
function Empty() {
  const [get] = useContext(EmptyContext);
  const now = get();
  return <span>{now}</span>;
}
export function EmptyApp() {
  const value = createSignal<string>();
  return (
    <EmptyContext value={value}>
      <Empty />
    </EmptyContext>
  );
}

// The function form over an async memo (Chrome: ASYNC_OUTSIDE_LOADING_BOUNDARY at the JSX read, no Loading): uncertifiable.
const SlowContext = createContext<Pair<string>>();
function Slow() {
  const [get] = useContext(SlowContext);
  return <span>{get()}</span>;
}
export function SlowApp() {
  const slow = createMemo(async () => {
    await Promise.resolve();
    return "s";
  });
  const value = createSignal(() => slow());
  return (
    <SlowContext value={value}>
      <Slow />
    </SlowContext>
  );
}

// The function form, synchronous: a derived signal all the same.
const DerivedContext = createContext<Pair<string>>();
function Derived() {
  const [get] = useContext(DerivedContext);
  const now = get();
  return <span>{now}</span>;
}
export function DerivedApp() {
  const value = createSignal(() => "d");
  return (
    <DerivedContext value={value}>
      <Derived />
    </DerivedContext>
  );
}

// A value the facts do not prove is no function (an identifier).
const NamedContext = createContext<Pair<string>>();
const initial = "n";
function Named() {
  const [get] = useContext(NamedContext);
  const now = get();
  return <span>{now}</span>;
}
export function NamedApp() {
  const value = createSignal(initial);
  return (
    <NamedContext value={value}>
      <Named />
    </NamedContext>
  );
}

// An options object.
const OptionsContext = createContext<Pair<string>>();
function Options() {
  const [get] = useContext(OptionsContext);
  const now = get();
  return <span>{now}</span>;
}
export function OptionsApp() {
  const value = createSignal("o", { name: "o" });
  return (
    <OptionsContext value={value}>
      <Options />
    </OptionsContext>
  );
}
