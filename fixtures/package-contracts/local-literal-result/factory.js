const factory = (token) => (key) => typeof key === "string" && key.startsWith(token);
const check = factory("--");
export function value(flag, key, input) {
  return check(key);
}
