function make(flag, input) {
  const result = {};
  if (flag) return input;
  return result;
}
export function value(flag, key, input) {
  const result = make(flag, input);
  return result[key];
}
