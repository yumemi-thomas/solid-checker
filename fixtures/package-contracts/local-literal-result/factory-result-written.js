const factory = (token) => (key) => key.startsWith(token);
let check = factory("--");
check = (key) => key;
export function value(flag, key, input) { return check(key); }
