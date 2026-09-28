// The tracer for ADR 0113: one `return` whose output is `plain`, closed over a
// completion the producer proved primitive. Every export is a plain function
// with no Solid call and no parameter read; what differs between them is only
// what the completion hands back to the caller.

// --- Every export in this block certifies `returns: [return plain]`. ---

// An expression body the checker types `boolean`.
export const trueFn = () => true;

// `void 0` is `undefined`, a primitive, and still a value-carrying
// completion: the valueless-completion walk declines `returns: []` on an
// expression body, so this is the closure that describes it.
export const voidFn = () => void 0;

// One return the checker types `number` from the default library alone,
// whatever the arguments are.
export function clamp(value, min, max) {
  return Math.min(Math.max(value, min), max);
}

// Two reachable returns, both strings.
export function label(flag) {
  if (flag) {
    return "on";
  }
  return "off";
}

// A comparison is a boolean whatever its operands are.
export function isObject(value) {
  return value !== null && typeof value === "object";
}

// A bare `return;` hands back `undefined`, a primitive, beside a value return.
export function sign(value) {
  if (value === 0) {
    return;
  }
  return value > 0 ? 1 : -1;
}

// A `const` holds its initializer's value for its whole life, so a read of one
// whose initializer is a literal is a primitive by grammar (the 2026-09-28
// amendment to ADR 0113).
const LIMIT = 10;
export function limit() {
  return LIMIT;
}

// --- Every export below is proposed the same closure and refused. ---

// The hole the 2026-09-28 amendment to ADR 0113 closes. In a JavaScript file
// the checker types `x` by its declaration, `number`, whatever the unchecked
// write stored, so the return site's type and `primitiveCompletion` both say
// primitive; and the veto's samples never pass the one key that stores the
// function. Only the evidence the census now requires beside the type -- the
// value's grammar, a reviewed built-in's call, or TypeScript source -- refuses
// it.
export function reassignedLet(key) {
  let x = 0;
  if (key === "unlock-the-function") {
    x = () => 1;
  }
  return x;
}

// An object: the one thing a primitive completion rules out.
export function box(value) {
  return { value };
}

// The caller's own argument. An unannotated JavaScript parameter is `any`,
// and `any` is not a primitive.
export function passThrough(value) {
  return value;
}

// A JSDoc `@returns` the checker takes at its word, over a body that hands
// back an object. The signature's type is the annotation; the return
// expression's own type is the body's, and that is what refuses it.
/** @returns {number} */
export function annotatedBox() {
  return {};
}

// `+` always yields a string, a number or a bigint at run time, but TypeScript
// types it `any` over untyped operands, and the census reads the type. This is
// the approximation ADR 0113 names: a primitive it cannot see.
export function add(a, b) {
  return a + b;
}

// A primitive body under a declaration that promises more: the census clears
// the body, and the operation's positive fact refuses the declared result.
export function widened() {
  return 1;
}
