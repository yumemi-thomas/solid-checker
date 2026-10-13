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
  const value = readWhen(5);
  return <main>{value}</main>;
}
export function CallsReadGuarded() {
  const value = readGuarded(false);
  return <main>{value}</main>;
}
export function CallsReadStoreGuarded() {
  const value = readStoreGuarded(false);
  return <main>{value}</main>;
}
export function CallsReadThroughWhen() {
  const value = readThroughWhen(false);
  return <main>{value}</main>;
}
export function CallsReadDefaultGuarded() {
  const value = readDefaultGuarded(false);
  return <main>{value}</main>;
}
export function CallsReadAfterThrowGuard() {
  const value = readAfterThrowGuard(true);
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
    value = readAfterFail(false);
  } catch {
    value = -1;
  }
  return <main>{value}</main>;
}
export function CallsReadAfterAssert() {
  let value = 0;
  try {
    value = readAfterAssert(false);
  } catch {
    value = -1;
  }
  return <main>{value}</main>;
}
export function CallsReadAfterFailingDefault() {
  let value = 0;
  try {
    value = readAfterFailingDefault(false);
  } catch {
    value = -1;
  }
  return <main>{value}</main>;
}
