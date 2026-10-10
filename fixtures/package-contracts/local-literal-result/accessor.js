function make() {
  const result = { get property() { return 1; } };
  return result;
}
export function value(flag, key, input) {
  const result = make();
  return result[key];
}
