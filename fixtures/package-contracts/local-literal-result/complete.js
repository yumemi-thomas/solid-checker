function make(flag) {
  const result = {};
  if (flag) return result;
  return result;
}
export function value(flag, key, input) {
  const result = make(flag);
  result[key] = input;
  return result[key];
}
