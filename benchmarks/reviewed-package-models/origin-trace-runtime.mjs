// Preserve registration frames across a synchronous callback invocation that
// happens later. This does not propagate origins through await or new promises.
export function collectPackageOrigins() {
  let active = null;
  const failures = [], registrations = new WeakMap(), recordedErrors = new WeakSet();
  function invoke(registration, callback, receiver, args) {
    const previous = active; active = registration;
    try { return Reflect.apply(callback, receiver, args); }
    catch (error) {
      if (registration && (!(error instanceof Object) || !recordedErrors.has(error))) {
        failures.push({ message: error instanceof Error ? error.message : String(error), registration });
        if (error instanceof Object) recordedErrors.add(error);
      }
      throw error;
    } finally { active = previous; }
  }
  return { failures, current: () => active,
    // The exact runtime's fire wrapper keeps the registration in scope while
    // the runtime validates a returned cleanup after callback() has returned.
    invokeSettled(callback, fire) { return invoke(registrations.get(callback) ?? null, fire, undefined, []); },
    wrap(callback, premise) {
    const previousLimit = Error.stackTraceLimit;
    let stack; try { Error.stackTraceLimit = 100; stack = new Error().stack ?? ""; } finally { Error.stackTraceLimit = previousLimit; }
    const frames = stack.split("\n").flatMap(line => {
      const match = line.match(/(?:at .*?\()?((?:file:\/\/|https?:\/\/|\/).*?):(\d+):(\d+)\)?$/);
      return match ? [{ path: match[1], line: Number(match[2]), column: Number(match[3]) }] : [];
    });
    const registration = { premise, frames };
    const wrapped = function (...args) { return invoke(registration, callback, this, args); };
    registrations.set(wrapped, registration); return wrapped;
  } };
}
