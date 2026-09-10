function make(flag, input) {
  let result = {};
  if (flag) result = input;
  return result;
}
export function value(flag, key, input) {
  const result = make(flag, input);
  return result[key];
}
