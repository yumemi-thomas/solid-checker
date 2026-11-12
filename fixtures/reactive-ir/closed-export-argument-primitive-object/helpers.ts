function makeScope() {
  return {
    take(value: unknown) {
      return value;
    },
  };
}
const scope = makeScope();

// Hands its parameter on to a member of an object a call returned: nothing resolves what `take` does with it.
export function record(value: unknown) {
  return scope.take(value);
}

// Forwards its own parameter: the argument of the `record` call is a parameter, not a literal.
export function relay(value: unknown) {
  return record(value);
}
