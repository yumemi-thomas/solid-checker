import {
  createArrowTokens,
  createGetterTokens,
  createNestedTokens,
  createRenamable,
  createSharedTokens,
  createTokens,
  registry,
  rename,
  wait,
  wrapTokens,
} from "./source";

// Clean: `createTokens` returns a fresh literal whose `waitForToken` is the
// method that runs, and it reads nothing.
export function FactoryResult() {
  const tokens = createTokens();
  const value = wait(tokens);
  return <div>{value}</div>;
}

// Clean: the same through a concise arrow body.
export function ArrowFactoryResult() {
  const tokens = createArrowTokens();
  const value = wait(tokens);
  return <div>{value}</div>;
}

// Uncertifiable: a getter's value is not the getter.
export function GetterResult() {
  const tokens = createGetterTokens();
  const value = wait(tokens);
  return <div>{value}</div>;
}

// Uncertifiable: one return is a caller's object.
export function SharedResult() {
  const tokens = createSharedTokens(false, createTokens());
  const value = wait(tokens);
  return <div>{value}</div>;
}

// Uncertifiable: the literal's own properties do not hold `waitForToken`.
export function NestedResult() {
  const tokens = createNestedTokens() as ReturnType<typeof createTokens>;
  const value = wait(tokens);
  return <div>{value}</div>;
}

// Uncertifiable: a same-named property in the call's argument says nothing
// about what the call returns.
export function ArgumentLiteral() {
  const tokens = wrapTokens({
    waitForToken() {
      return 8;
    },
    settle() {},
  });
  const value = wait(tokens);
  return <div>{value}</div>;
}

// Uncertifiable: a method call is selected by its receiver.
export function MethodResult() {
  const tokens = registry.create();
  const value = wait(tokens);
  return <div>{value}</div>;
}

// Uncertifiable: a `let` may hold another object by the time of the call.
export function MutableBinding() {
  let tokens = createTokens();
  const value = wait(tokens);
  return <div>{value}</div>;
}

// Uncertifiable: a destructured name is a member of the initializer.
export function DestructuredBinding() {
  const { inner } = { inner: createTokens() };
  const value = wait(inner);
  return <div>{value}</div>;
}

// Uncertifiable: the program assigns a `rename` member.
export function ReassignedMember() {
  const value = rename(createRenamable());
  return <div>{value}</div>;
}
