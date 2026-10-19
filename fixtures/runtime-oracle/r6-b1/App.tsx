import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
type Runner = { run: () => void };
type Box = { inner: Runner };
function helper(o: Runner) {
  o.run();
}
declare const src: object;

export function Control() {
  helper(opaque);
  return <main />;
}
export function Resolved() {
  helper({ run() {} });
  return <main />;
}
// Nested object replaced through Object.assign on the nested member.
export function NestedAssignOpaque() {
  const o: Box = { inner: { run() {} } };
  Object.assign(o.inner, opaque);
  helper(o.inner);
  return <main />;
}
export function NestedAssignLiteral() {
  const o: Box = { inner: { run() {} } };
  Object.assign(o.inner, { run: opaque.run });
  helper(o.inner);
  return <main />;
}
// for-in copier into the nested member.
export function NestedForIn() {
  const o: Box = { inner: { run() {} } };
  const target = o.inner as Record<string, unknown>;
  for (const k in src) target[k] = (src as Record<string, unknown>)[k];
  helper(o.inner);
  return <main />;
}
// defineProperty on the nested member.
export function NestedDefine() {
  const o: Box = { inner: { run() {} } };
  Object.defineProperty(o.inner, "run", { value: opaque.run });
  helper(o.inner);
  return <main />;
}
// Class: constructor returns another object.
class Swapper {
  constructor() {
    return opaque as unknown as Swapper;
  }
  run() {}
}
export function CtorReturns() {
  helper(new Swapper());
  return <main />;
}
// Class: getter named run returns an opaque function.
class Getter {
  get run() {
    return opaque.run;
  }
}
export function ClassGetter() {
  helper(new Getter());
  return <main />;
}
// Literal getter (refused by ADR 0320 rule 2) as a control.
export function LiteralGetter() {
  helper({ get run() { return opaque.run; } });
  return <main />;
}
// Mixin base.
declare function mix<T extends new () => object>(base: T): new () => Runner;
class Base {
  run() {}
}
class Mixed extends mix(Base) {}
export function Mixin() {
  helper(new Mixed());
  return <main />;
}
// Subclass overriding in the same file with an opaque method.
class Sub extends Base {
  run = opaque.run;
}
export function SubField() {
  helper(new Sub());
  return <main />;
}
// Function constructor with a prototype object.
function F(this: object) {}
F.prototype = { run() {} };
export function ProtoObject() {
  helper(new (F as unknown as new () => Runner)());
  return <main />;
}
// Object.create of a prototype.
export function Created() {
  helper(Object.create({ run() {} }) as Runner);
  return <main />;
}
// Shared module object returned by a factory and mutated through the factory.
const shared = { run() {} };
function make(): Runner {
  return shared;
}
export function SharedFactory() {
  Object.assign(make(), opaque);
  helper(make());
  return <main />;
}
// Memoized factory.
let cache: Runner | undefined;
function memo(): Runner {
  return (cache ??= { run() {} });
}
export function Memo() {
  memo().run = opaque.run;
  helper(memo());
  return <main />;
}
// Async factory awaited.
async function later(): Promise<Runner> {
  return { run() {} };
}
export async function AsyncFactory() {
  helper(await later());
}
