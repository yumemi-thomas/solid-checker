const factory = (token) => (key, input) => {
  token = input;
  return key.startsWith(token);
};
const check = factory("--");
export function value(flag, key, input) {
  return check(key, input);
}
