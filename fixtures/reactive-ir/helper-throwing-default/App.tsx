import { createSignal } from "solid-js";

// ADR 1010: a helper that calls its callback in its body runs it only if the body runs. A default that may run code runs first whenever
// its argument is missing or `undefined`, and one that throws ends the call before the body: the callback's write is not proven to run.
const stopGate = (): number => {
  throw new Error("gate");
};

// Violation: the default is a literal, the body runs and calls the callback, which writes in the component body.
export function InertDefault() {
  const [, setW] = createSignal(0);
  const gated = (g: () => void, _gate: number = 1) => {
    g();
  };
  gated(() => {
    setW(7);
  });
  return <div />;
}

// Violation: the throwing default does not run, its argument is a literal.
export function DefaultSupplied() {
  const [, setW] = createSignal(0);
  const gated = (g: () => void, _gate: number = stopGate()) => {
    g();
  };
  gated(() => {
    setW(7);
  }, 1);
  return <div />;
}

// Obligation: the default throws before the body, so the callback never runs (Chrome is silent).
export function ThrowingDefault() {
  const [, setW] = createSignal(0);
  const gated = (g: () => void, _gate: number = stopGate()) => {
    g();
  };
  try {
    gated(() => {
      setW(7);
    });
  } catch {
    // the default threw
  }
  return <div />;
}

// Obligation: the same through a `function` declaration.
function gatedDeclared(g: () => void, _gate: number = stopGate()) {
  g();
}
export function ThrowingDefaultDeclared() {
  const [, setW] = createSignal(0);
  try {
    gatedDeclared(() => {
      setW(7);
    });
  } catch {
    // the default threw
  }
  return <div />;
}

// Obligation: the argument is a name that may hold `undefined`, so the default may run.
const maybe: number | undefined = undefined as number | undefined;
export function DefaultMaybeSupplied() {
  const [, setW] = createSignal(0);
  const gated = (g: () => void, _gate: number = stopGate()) => {
    g();
  };
  try {
    gated(() => {
      setW(7);
    }, maybe);
  } catch {
    // the default threw
  }
  return <div />;
}
