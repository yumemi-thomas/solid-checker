// The tracer for ADR 0152: an export that hands its caller a fresh function
// literal which calls an argument the export was handed. The literal's own
// call claims gain a `callbacks` item per such argument, and the export's own
// `callbacks` keeps each one at `result-access`: it runs only when the
// returned value is invoked, on that invoker's stack.

// --- Every export in this block certifies its nested items. ---

// `@solid-primitives/utils`' `pipe`, byte for byte: both arguments run once,
// in order, on every invocation, and the second one's result is handed back.
export function pipe(a, b) {
  return (raw) => b(a(raw));
}

// `@solid-primitives/promise`' `changed`, byte for byte: `source` runs once
// per invocation; the completion is a primitive by grammar.
export function changed(source, times = 1) {
  times += 1;
  return () => {
    source();
    return !--times;
  };
}

// A throw before the call leaves every normal completion running it.
export function required(callback, message) {
  return () => {
    if (message === undefined) throw "missing message";
    callback();
  };
}

// --- Every export below proposes an item and is withdrawn by name. ---

// Conditionally: `callback && callback()` runs it on some invocations only.
export function guarded(callback) {
  return () => {
    callback && callback();
  };
}

// Twice per invocation: no item states a count of two.
export function twice(callback) {
  return () => {
    callback();
    callback();
  };
}

// From a callable nested in the literal, later: not on the invoker's stack.
export function deferred(callback) {
  return () => {
    queueMicrotask(() => callback());
  };
}

// An early return before the call leaves some completions without it.
export function early(callback, flag) {
  return () => {
    if (flag) return;
    callback();
  };
}

// A defaulted argument: when it is omitted the literal calls the package's own
// fallback, which is not the caller's argument.
const fallback = () => undefined;
export function defaulted(callback = fallback) {
  return () => {
    callback();
  };
}

// The export keeps the argument somewhere else as well: its `callbacks`
// cannot say it is kept only by the returned value.
const registry = [];
export function registered(callback) {
  registry.push(callback);
  return () => {
    callback();
  };
}
