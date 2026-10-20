// Calls its parameter on every completion.
export function runAlways(cb: () => number) {
  return cb();
}

// Calls it only when asked.
export function runWhen(flag: boolean, cb: () => number) {
  if (flag) return cb();
  return 0;
}

// Calls it as a ternary arm.
export function callWhen(cb: () => number, flag: boolean) {
  return flag ? cb() : 0;
}

// Calls it once per item, so not at all for an empty list.
export function runEach(items: number[], cb: () => number) {
  let total = 0;
  for (const item of items) total += item + cb();
  return total;
}

// Calls it only when the target is unset.
export function runWhenUnset(target: { value?: number }, cb: () => number) {
  target.value ??= cb();
  return target.value;
}

export function fail(message: string): never {
  throw new Error(message);
}

// A `never`-returning guard before the call: a caller may catch it.
export function runAfterFail(ready: boolean, cb: () => number) {
  if (!ready) fail("not ready");
  return cb();
}

// The parameter is replaced before it is called.
export function runReassigned(cb: () => number) {
  cb = () => 0;
  return cb();
}

// Calls its second parameter on every completion.
export function callSecond(_first: unknown, cb: () => number, ..._rest: unknown[]) {
  return cb();
}

// A helper binding that may be replaced.
export let runner = (cb: () => number) => cb();
export function disableRunner() {
  runner = () => 0;
}

export class Base {
  run(cb: () => number) {
    return cb();
  }
}
export class Quiet extends Base {
  override run(_cb: () => number) {
    return 0;
  }
}
export function makeBase(quiet: boolean): Base {
  return quiet ? new Quiet() : new Base();
}

// An object literal method nobody reassigns.
export const bus = {
  run(cb: () => number) {
    return cb();
  },
};
