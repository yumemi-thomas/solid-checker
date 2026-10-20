import { createSignal } from "solid-js";
import { readAfterGuard, readFallback, readGuarded, readWhen, readWritten, runWhen } from "./helpers";

declare const unknownFlag: boolean;
declare const either: number | (() => number);

// ---- Proven by the arguments: the read always runs.

export function WhenFunction() {
  const value = readWhen(() => 1);
  return <main>{value}</main>;
}
export function GuardedTrue() {
  const value = readGuarded(true);
  return <main>{value}</main>;
}
export function FallbackOmitted() {
  const value = readFallback();
  return <main>{value}</main>;
}
export function AfterGuardReady() {
  const value = readAfterGuard(true);
  return <main>{value}</main>;
}
export function PassedWhenTrue() {
  const [local] = createSignal(1);
  const value = runWhen(true, local);
  return <main>{value}</main>;
}

// ---- Ruled out by the arguments: no read happens, nothing is reported.

export function WhenNumber() {
  const value = readWhen(5);
  return <main>{value}</main>;
}
export function GuardedFalse() {
  const value = readGuarded(false);
  return <main>{value}</main>;
}
export function FallbackGiven() {
  const value = readFallback(1);
  return <main>{value}</main>;
}
export function PassedWhenFalse() {
  const [local] = createSignal(1);
  const value = runWhen(false, local);
  return <main>{value}</main>;
}

// ---- Undecided: uncertifiable, as without argument facts.

export function WhenEither() {
  const value = readWhen(either);
  return <main>{value}</main>;
}
export function GuardedUnknown() {
  const value = readGuarded(unknownFlag);
  return <main>{value}</main>;
}
export function WrittenTrue() {
  const value = readWritten(true);
  return <main>{value}</main>;
}
export function PassedWhenUnknown() {
  const [local] = createSignal(1);
  const value = runWhen(unknownFlag, local);
  return <main>{value}</main>;
}
export function AfterGuardNotReady() {
  let value = 0;
  try {
    value = readAfterGuard(false);
  } catch {
    value = -1;
  }
  return <main>{value}</main>;
}
