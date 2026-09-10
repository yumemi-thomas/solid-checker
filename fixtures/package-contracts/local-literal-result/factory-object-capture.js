const factory = (object) => (key) => object[key];
const check = factory({});
export function value(flag, key, input) { return check(key); }
