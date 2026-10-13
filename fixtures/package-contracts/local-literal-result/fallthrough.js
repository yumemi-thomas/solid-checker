function make(flag) {
  const result = {};
  if (flag) return result;
}
export function value(flag, key, input) {
  const result = make(flag);
  return result[key];
}
