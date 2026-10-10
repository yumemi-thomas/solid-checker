let factory = (token) => (key) => key.startsWith(token);
factory = (token) => (key) => key.endsWith(token);
const check = factory("--");
export function value(flag, key, input) { return check(key); }
