import { action, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);

// Control: the declaration form, always reachable by its own name.
function declaredAction() {
  const run = action(function* () {});
  void run();
  return null;
}
export function DeclaredCaller() {
  declaredAction();
  return <button>declared</button>;
}

// Positive: an arrow bound to a `const`, called directly in a component body.
const arrowAction = () => {
  const run = action(function* () {});
  void run();
  return null;
};
export function ArrowCaller() {
  arrowAction();
  return <button>arrow</button>;
}

// Positive: a function expression bound to a `const`.
const expressionAction = function () {
  const run = action(function* () {});
  void run();
  return null;
};
export function ExpressionCaller() {
  expressionAction();
  return <button>expression</button>;
}

// Positive: a named function expression bound to a `const` is reached through
// the binding, not through its own name.
const namedExpressionAction = function inner() {
  const run = action(function* () {});
  void run();
  return null;
};
export function NamedExpressionCaller() {
  namedExpressionAction();
  return <button>named expression</button>;
}

// Control: the declaration form of a write helper.
function bumpDeclared() {
  setCount(1);
}
export function WriteDeclaredCaller() {
  bumpDeclared();
  return <button>write declared</button>;
}

// Positive: an untracked signal read in an arrow helper run in setup.
const readCount = () => count();
export function ReadCaller() {
  const snapshot = readCount();
  return <button>{snapshot}</button>;
}

// Positive: a signal write in an arrow helper run in setup.
const bump = () => {
  setCount(1);
};
export function WriteCaller() {
  bump();
  return <button>write</button>;
}

// Positive: a wrapper chain, component -> arrow helper -> arrow helper.
const innerChain = () => {
  const run = action(function* () {});
  void run();
  return null;
};
const outerChain = () => innerChain();
export function ChainCaller() {
  outerChain();
  return <button>chain</button>;
}

// Negative: the helper's only caller is an event handler, which is not owned
// setup.
const handlerOnly = () => {
  const run = action(function* () {});
  void run();
};
export function HandlerCaller() {
  return <button onClick={() => handlerOnly()}>handler</button>;
}

// Negative: nothing calls the helper, so nothing proves it runs.
const neverCalled = () => {
  const run = action(function* () {});
  void run();
};

// Negative: the component's own `shadowedHelper` binding shadows the module
// helper, so the call reaches the local value, not the arrow.
const shadowedHelper = () => {
  const run = action(function* () {});
  void run();
  return null;
};
const noop = () => null;
export function ShadowCaller() {
  const shadowedHelper = noop;
  shadowedHelper();
  return <button>shadow</button>;
}

// Negative: a same-named arrow declared inside the component shadows the module
// helper; the module helper is never reached, and the local one is harmless.
const shadowedByLocalArrow = () => {
  const run = action(function* () {});
  void run();
  return null;
};
export function LocalArrowShadowCaller() {
  const shadowedByLocalArrow = () => null;
  shadowedByLocalArrow();
  return <button>local arrow shadow</button>;
}

// Negative: a `let` that is reassigned is not an exact binding, so the call
// may reach either function.
let reassignedHelper = () => {
  const run = action(function* () {});
  void run();
  return null;
};
reassignedHelper = () => null;
export function ReassignedCaller() {
  reassignedHelper();
  return <button>reassigned</button>;
}
