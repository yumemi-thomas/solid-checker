// The tracer for ADR 0145: an export that hands its caller a fresh function
// literal proposes `returns` closed over one `return` whose output is a
// described callable -- the literal's own call claims. What differs between
// the exports is only what the returned literal does when it is called.

// --- Every export in this block certifies a described callable. ---

// `@solid-primitives/utils`' `createIdGenerator`, byte for byte: the returned
// literal reads and writes captured plain bindings and calls the default
// library only, and hands back a string.
export const createIdGenerator = () => {
  let seq = 0;
  const rand = Math.random().toString(36).slice(2, 8);
  return () => `${Date.now().toString(36)}-${(++seq).toString(36)}-${rand}`;
};

// A returned literal that completes without a value: `returns: []`.
export function makeNoop() {
  return () => {};
}

// An expression-bodied factory of a valueless literal. The test also plans it
// claimed `returns: [plain]`, which its body does not show.
export const makeSilent = () => () => {};

// A counter over a captured `let`: the increment is a number by its grammar.
export function makeTicker() {
  let ticks = 0;
  return function next() {
    return ++ticks;
  };
}

// A conditional of two literals with the same claims is one described
// callable, handed back by both arms.
export function choose(flag) {
  return flag ? () => 1 : () => "one";
}

// --- Every export below proposes the same shape and is withdrawn by name. ---

// The returned literal calls the caller's own argument to the export, which
// it captured: calling it runs the caller's code.
export function invokesCaptured(fn) {
  return () => fn();
}

// The returned literal calls its own argument: its caller's code.
export function invokesOwnArgument() {
  return (callback) => callback();
}

// The returned literal hands back an object, not a primitive.
export function returnsObject() {
  return () => ({});
}

// The same counter returning the binding itself. The checker types the read
// `number`, from the declaration, and in a JavaScript file an unchecked write
// never widens that, so the type is no proof: withdrawn.
export function makeCounter() {
  let count = 0;
  return function next() {
    count += 1;
    return count;
  };
}

// The returned literal reads a member of a value the caller handed the
// export, which may run a getter.
export function readsCapturedMember(options) {
  return () => options.value === 1;
}

// --- Proposed nothing: the returned value is not a literal. ---

// Returned through a binding the body writes: which function it holds is not
// the literal's identity, so no described callable is proposed.
export function throughMutableBinding(flag) {
  let next = () => 1;
  if (flag) next = () => ({});
  return next;
}
