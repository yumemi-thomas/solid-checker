// The tracer for the `returns` implementation census (ADR 0035): whether one
// invocation of an export completes without yielding a value to its caller.
// Every export here is a plain consumer function with no Solid call, so its
// `creates` also proposes and certifies; what differs between them is only the
// completion.

let seen = 0;

function work() {
  seen += 1;
}

// A body that falls off its end: the completion the claim describes.
export function bareCompletion(count) {
  seen += count;
}

// A bare `return;` on one path and the end of the body on the other. Neither
// carries a value.
export function earlyBareReturn(flag) {
  if (!flag) {
    return;
  }
  work();
}

// A bare `return;` inside a loop: the producer's control-flow census marks the
// loop `reachability-lower-bound`, which is the one admissible incompleteness,
// and the return site inside it is enumerated with reach `unknown`.
export function bareReturnInLoop(count) {
  for (let index = 0; index < count; index += 1) {
    if (index === 3) {
      return;
    }
    work();
  }
}

// A nested callable that returns a value. That completion is `inner`'s, not
// this export's: the value reaches the caller only if this export returns it,
// which it does not.
export function nestedReturnsValue(count) {
  function inner(value) {
    return value + 1;
  }
  seen = inner(count);
}

// --- Every export below yields a value to its caller, and refuses by name. ---

// A `return` carrying an expression.
export function returnsValue(value) {
  return value;
}

// An expression-bodied arrow completes with its expression.
export const expressionArrow = (value) => value;

// An `async` function hands its caller a promise on every completion, whatever
// the body does.
export async function asyncVoid() {
  work();
}

// A generator hands its caller an iterator on every completion.
export function* generatorVoid() {
  work();
}

// A value-carrying `return` inside a loop is still a value-carrying
// completion, with reach `unknown`.
export function valueReturnInLoop(count) {
  for (let index = 0; index < count; index += 1) {
    if (index === 3) {
      return index;
    }
  }
}
