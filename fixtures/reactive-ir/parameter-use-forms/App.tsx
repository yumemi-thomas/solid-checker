type Runner = { run: () => void };

// `helper` invokes a member of its parameter; every function below hands
// its own parameter on in a form no call site can resolve, so the dispatch
// is an obligation at that function's callers.
function helper(o: Runner) {
  o.run();
}
function helperBox(box: { inner: Runner }) {
  box.inner.run();
}
function helperList(list: Runner[]) {
  list[0].run();
}

function viaAlias(o: Runner) {
  const alias = o;
  helper(alias);
}
export function Alias() {
  viaAlias(opaque);
  return <main />;
}
function viaSpread(o: Runner) {
  helper({ ...o });
}
export function Spread() {
  viaSpread(opaque);
  return <main />;
}
function viaWrapper(o: Runner) {
  helperBox({ inner: o });
}
export function Wrapper() {
  viaWrapper(opaque);
  return <main />;
}
function viaArray(o: Runner) {
  helperList([o]);
}
export function ArrayElement() {
  viaArray(opaque);
  return <main />;
}
function viaConditional(o: Runner, flag: boolean) {
  helper(flag ? o : o);
}
export function Conditional() {
  viaConditional(opaque, true);
  return <main />;
}
function viaArguments(o: Runner) {
  // eslint-disable-next-line prefer-rest-params
  helper(arguments[0] as Runner);
  void o;
}
export function ArgumentsObject() {
  viaArguments(opaque);
  return <main />;
}
function viaClosure(o: Runner) {
  return () => helper(o);
}
export function StoredClosure() {
  viaClosure(opaque)();
  return <main />;
}

// A member read off the parameter and called elsewhere.
function destructured(o: Runner) {
  const { run } = o;
  run();
}
export function Destructured() {
  destructured(opaque);
  return <main />;
}
function extracted(o: Runner) {
  const run = o.run;
  run();
}
export function Extracted() {
  extracted(opaque);
  return <main />;
}
function scheduled(o: Runner) {
  queueMicrotask(o.run);
}
export function HandedMember() {
  scheduled(opaque);
  return <main />;
}
function destructuredParameter({ run }: Runner) {
  run();
}
export function DestructuredParameter() {
  destructuredParameter(opaque);
  return <main />;
}

// Controls: a test or arithmetic use hands nothing on, and a primitive
// argument carries no member.
function tested(o: Runner | undefined) {
  if (o) {
    return typeof o === "object";
  }
  return false;
}
export function TestedOnly() {
  tested(opaque);
  return <main />;
}
function scaled(value: number) {
  return Math.max(value * 2, 0);
}
export function PrimitiveArgument() {
  scaled(3);
  return <main />;
}
