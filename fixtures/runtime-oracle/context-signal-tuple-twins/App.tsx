import { createContext, createSignal, useContext, type Accessor, type Setter } from "solid-js";

type Pair = readonly [Accessor<string>, Setter<string>];

// Each consumer reads its slot in the body, which a proof would make SC1001; every one stays uncertifiable.

// Two providers of one context.
const TwoContext = createContext<Pair>();
function Two() {
  const [get] = useContext(TwoContext);
  const now = get();
  return <span>{now}</span>;
}
export function TwoApp() {
  const a = createSignal("a");
  const b = createSignal("b");
  return (
    <TwoContext value={a}>
      <TwoContext value={b}>
        <Two />
      </TwoContext>
    </TwoContext>
  );
}

// The context escapes: a function receives it.
const EscapedContext = createContext<Pair>();
function keep(context: unknown) {
  return context;
}
keep(EscapedContext);
function Escaped() {
  const [get] = useContext(EscapedContext);
  const now = get();
  return <span>{now}</span>;
}
export function EscapedApp() {
  const value = createSignal("e");
  return (
    <EscapedContext value={value}>
      <Escaped />
    </EscapedContext>
  );
}

// Another consumer holds the shared tuple whole, so it could replace a slot.
const HeldContext = createContext<Pair>();
function Holder() {
  const pair = useContext(HeldContext);
  return <span>{pair.length}</span>;
}
function Held() {
  const [get] = useContext(HeldContext);
  const now = get();
  return <span>{now}</span>;
}
export function HeldApp() {
  const value = createSignal("h");
  return (
    <HeldContext value={value}>
      <Holder />
      <Held />
    </HeldContext>
  );
}

// A default: a read under no provider answers the default, not the signal.
const fallback: Pair = [() => "d", ((v: string) => v) as unknown as Setter<string>];
const DefaultContext = createContext<Pair>(fallback);
function Defaulted() {
  const [get] = useContext(DefaultContext);
  const now = get();
  return <span>{now}</span>;
}
export function DefaultApp() {
  const value = createSignal("d");
  return (
    <DefaultContext value={value}>
      <Defaulted />
    </DefaultContext>
  );
}

// Exported: another module may provide it.
export const ExportedContext = createContext<Pair>();
function Exported() {
  const [get] = useContext(ExportedContext);
  const now = get();
  return <span>{now}</span>;
}
export function ExportedApp() {
  const value = createSignal("x");
  return (
    <ExportedContext value={value}>
      <Exported />
    </ExportedContext>
  );
}

// The provided signal is also read elsewhere: its binding has another reference.
const SharedContext = createContext<Pair>();
function Shared() {
  const [get] = useContext(SharedContext);
  const now = get();
  return <span>{now}</span>;
}
export function SharedApp() {
  const value = createSignal("s");
  const other = value;
  return (
    <SharedContext value={other}>
      <Shared />
    </SharedContext>
  );
}

// A project function named `useContext` (shadowing) is no context read.
const ShadowContext = createContext<Pair>();
function Shadow() {
  const useContext = (_context: unknown): Pair => [() => "z", ((v: string) => v) as unknown as Setter<string>];
  const [get] = useContext(ShadowContext);
  const now = get();
  return <span>{now}</span>;
}
export function ShadowApp() {
  const value = createSignal("z");
  return (
    <ShadowContext value={value}>
      <Shadow />
    </ShadowContext>
  );
}

// `children` given as a prop goes on as any component's prop (a package may run it); written children are any component's children too.
const FnContext = createContext<Pair>();
function Shown() {
  const [label] = createSignal("s");
  return <span>{label()}</span>;
}
export function FnChildApp() {
  const value = createSignal("f");
  return <FnContext value={value} children={<Shown />} />;
}
