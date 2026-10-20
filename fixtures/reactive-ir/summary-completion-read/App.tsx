import {
  readAfterLoop,
  readAfterAssert,
  readAfterFail,
  readAfterFailingDefault,
  readAfterThrowGuard,
  readAlways,
  readDefaultGuarded,
  readGuarded,
  readStoreGuarded,
  readThroughAlways,
  readThroughWhen,
  readWhen,
  readWhenUnset,
} from "./helpers";

// Arguments that prove nothing, so these cases test the rule without
// argument facts (ADR 0280 is pinned by `argument-sensitive-reads`).
declare const flag: boolean;
declare const either: number | (() => number);

// ---- Proven: the read runs on every normal completion of the call.

export function CallsReadAlways() {
  const value = readAlways();
  return <main>{value}</main>;
}
export function CallsReadThroughAlways() {
  const value = readThroughAlways();
  return <main>{value}</main>;
}

// ---- Not proven: the read is on some paths of the call only.

export function CallsReadWhen() {
  const value = readWhen(either);
  return <main>{value}</main>;
}
export function CallsReadGuarded() {
  const value = readGuarded(flag);
  return <main>{value}</main>;
}
export function CallsReadStoreGuarded() {
  const value = readStoreGuarded(flag);
  return <main>{value}</main>;
}
export function CallsReadThroughWhen() {
  const value = readThroughWhen(flag);
  return <main>{value}</main>;
}
export function CallsReadDefaultGuarded() {
  const value = readDefaultGuarded(flag);
  return <main>{value}</main>;
}
export function CallsReadAfterThrowGuard() {
  const value = readAfterThrowGuard(flag);
  return <main>{value}</main>;
}
export function CallsReadWhenUnset() {
  const value = readWhenUnset({ value: 1 });
  return <main>{value}</main>;
}
export function CallsReadAfterLoop() {
  const value = readAfterLoop([]);
  return <main>{value}</main>;
}
export function CallsReadAfterFail() {
  let value = 0;
  try {
    value = readAfterFail(flag);
  } catch {
    value = -1;
  }
  return <main>{value}</main>;
}
export function CallsReadAfterAssert() {
  let value = 0;
  try {
    value = readAfterAssert(flag);
  } catch {
    value = -1;
  }
  return <main>{value}</main>;
}
export function CallsReadAfterFailingDefault() {
  let value = 0;
  try {
    value = readAfterFailingDefault(flag);
  } catch {
    value = -1;
  }
  return <main>{value}</main>;
}
