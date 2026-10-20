function readNow(source: () => string) {
  return source();
}

function readMaybe(flag: boolean, source: () => string) {
  if (flag) return source();
  return "";
}

// Positive: the parameter is passed on, unchanged, to a helper that calls it
// on every completion.
export function useForwarded(source: () => string) {
  return readNow(source);
}

// Positive: a chain of such forwards composes.
export function useChained(source: () => string) {
  return useForwarded(source);
}

// Refusals: the forward is behind a branch, deferred, in a timer callback,
// or the helper calls its parameter only on some paths.
export function useGuarded(flag: boolean, source: () => string) {
  if (flag) readNow(source);
  return "";
}

export function useDeferred(source: () => string) {
  return () => readNow(source);
}

export function useMaybe(flag: boolean, source: () => string) {
  return readMaybe(flag, source);
}

export function useTimer(source: () => string) {
  setTimeout(() => readNow(source));
  return "";
}

// Negative: another value is forwarded, not the parameter.
export function useReplaced(source: () => string) {
  const other = () => "x";
  return readNow(other);
}

// Refusal: a destructured parameter before it shifts the summary's parameter
// positions, so the forward is not admitted.
export function useAfterPattern({ label }: { label: string }, source: () => string) {
  return label + readNow(source);
}

// Refusal: a `const` arrow target may be in its temporal dead zone when the
// forward runs (ADR 0218), so only function declarations are forward targets.
const readArrow = (source: () => string) => source();

export function useArrowTarget(source: () => string) {
  return readArrow(source);
}
