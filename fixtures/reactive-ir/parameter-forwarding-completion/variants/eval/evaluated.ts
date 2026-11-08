function readNow(source: () => string) {
  return source();
}

// Refusal: direct `eval` may replace the parameter before it is forwarded.
export function useEvaluated(source: () => string) {
  eval("source = () => 'x'");
  return readNow(source);
}
