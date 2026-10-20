type Runner = { run: () => void };

// `helper` invokes a member of its parameter: each call site's argument
// decides which implementation runs.
function helper(o: Runner) {
  o.run();
}

// ---- Clean: every alternative resolves to an inert implementation.

export function Resolved() {
  helper({ run() {} });
  return <main />;
}
export function TwoLiterals(props: { c: boolean }) {
  helper(props.c ? { run() {} } : { run() {} });
  return <main />;
}
// An own key after a spread replaces whatever the spread copied.
export function SpreadBefore() {
  helper({ ...partial, run() {} });
  return <main />;
}
export function ConstLiteral() {
  const held = { run() {} };
  helper(held);
  return <main />;
}
class Box {
  run() {}
}
export function ClosedInstance() {
  const box = new Box();
  helper(box);
  return <main />;
}
function first(list: { map: (f: (x: number) => number) => number[] }) {
  return list.map((x) => x);
}
export function ClosedArray() {
  const list = [1, 2];
  first(list);
  return <main />;
}

// ---- Uncertifiable: some alternative does not resolve, so none is trusted.

export function Conditional(props: { c: boolean }) {
  helper(props.c ? { run() {} } : opaque);
  return <main />;
}
export function ConstConditional(props: { c: boolean }) {
  const held = props.c ? { run() {} } : opaque;
  helper(held);
  return <main />;
}
export function Cast(props: { c: boolean }) {
  helper((props.c ? { run() {} } : opaque) as Runner);
  return <main />;
}
export function Nullish(props: { a?: Runner }) {
  helper(props.a ?? { run() {} });
  return <main />;
}
export function Or(props: { a?: Runner }) {
  helper(props.a || { run() {} });
  return <main />;
}
// A spread after the own key may replace it.
export function SpreadAfter() {
  helper({ run() {}, ...partial });
  return <main />;
}
export function SpreadOnly() {
  helper({ ...opaque });
  return <main />;
}
export function Reassigned(props: { c: boolean }) {
  let held: Runner = { run() {} };
  if (props.c) held = opaque;
  helper(held);
  return <main />;
}
export function Factory() {
  helper(makeOpaque());
  return <main />;
}
function pick(c: boolean): Runner {
  return c ? { run() {} } : opaque;
}
export function FactoryConditional(props: { c: boolean }) {
  helper(pick(props.c));
  return <main />;
}

// ---- Uncertifiable: the object is shared with code that may replace the member.

export function MutatedByCall() {
  const held = { run() {} };
  mutate(held);
  helper(held);
  return <main />;
}
export function EscapedInstance() {
  const box = new Box();
  mutate(box);
  helper(box);
  return <main />;
}
export function EscapedArray() {
  const list = [1, 2];
  mutate(list);
  first(list);
  return <main />;
}
// A method that uses its receiver may hand the object on.
export function ReceiverMethod() {
  const held: Runner & { share(): void } = {
    run() {},
    share() {
      mutate(this);
    },
  };
  helper(held);
  return <main />;
}
// The callee itself hands the object to unknown code before calling it.
function leaky(o: Runner) {
  mutate(o);
  o.run();
}
export function LeakyCallee() {
  leaky({ run() {} });
  return <main />;
}

// ---- Uncertifiable: the dispatch is decided by a caller of a helper.

// `forward` hands its own parameter to `helper`; the member `helper` calls is
// chosen by `forward`'s caller.
function forward(o: Runner) {
  helper(o);
}
export function Forwarded() {
  forward(opaque);
  return <main />;
}
// Control: the same forwarding with an inert argument is clean.
export function ForwardedResolved() {
  forward({ run() {} });
  return <main />;
}
// The parameter whose member runs is the second; the first is destructured.
function shifted({ run }: Runner, o: Runner) {
  void run;
  o.run();
}
export function ShiftedParameter() {
  shifted({ run() {} }, opaque);
  return <main />;
}
