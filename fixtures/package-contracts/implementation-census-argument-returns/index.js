// The tracer for ADR 0115 and ADR 0116: returns that each hand back the
// caller's own argument, a fresh array of the caller's arguments, or what an
// invocation of one returned. No export calls Solid; what differs is only which
// values the completions hand back.

// --- Every export in this block certifies its returns. ---

// `@solid-primitives/utils`' own `asArray`: the caller's value when it is an
// array, a fresh one-element array of it otherwise, and a fresh empty array
// for a falsy one.
export const asArray = (value) => Array.isArray(value) ? value : value ? [value] : [];

// Either of two arguments, whichever the flag selects.
export function pick(flag, left, right) {
  return flag ? left : right;
}

// Two statement returns: the value itself, or a fresh pair.
export function pairOrValue(value, other) {
  if (other === undefined) {
    return value;
  }
  return [value, other];
}

// ADR 0116. `@solid-primitives/utils`' own `accessWith` and `access`: the
// caller's value, or what calling it returned.
export function accessWith(valueOrFn, ...args) {
  return typeof valueOrFn === "function" ? valueOrFn(...args) : valueOrFn;
}
export const access = (v) => typeof v === "function" && !v.length ? v() : v;

// One invocation result alone, and one fresh array alone.
export const run = (fn, value) => fn(value);
export const wrap = (value) => [value];

// A parenthesized conditional is the conditional itself.
export const parenthesized = (value) => (Array.isArray(value) ? value : [value]);

// --- Every export below is refused. ---

// The parameter is written before it is returned, so no arm is the caller's
// own value any more.
export function reassigned(value) {
  value = value || [];
  return value ? value : [value];
}

// An element that is not an argument.
export function withLiteral(value) {
  return value ? value : [value, 1];
}

// A container no completion hands back, when a claim names one.
export function overclaimed(value) {
  return value ? value : [value];
}

// An arm that is a call's result.
export function viaCall(value) {
  return value ? value : Array.from(value);
}

// An optional call hands back `undefined` for a nullish callee, which no
// invocation returned.
export function optionalCall(fn, value) {
  return value ? fn?.() : value;
}
