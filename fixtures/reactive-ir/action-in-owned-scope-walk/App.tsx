import "fz-aug2";
import "fz-ctor";
import { listener, makeVoid } from "fz-typed";
import { makeMap } from "fz-target";
import { runStep } from "fz-step";
import { vendorStep as viaBarrel } from "./re-shim";
import { vendorStep as viaQuery } from "./step.vendor.js?fz";
import * as re from "./re";
import * as outer from "./re2";
import * as local from "./local";
import { localFn } from "./local";

// Values a package made, written at module scope where no owner is active: the row stands where the value is called.
const n = () => {};
const madeMap = makeMap();
const proxied = new Proxy(makeMap(), {});
const plainProxy = new Proxy({ run() {} }, {});
const nsAlias = re;
const { reRun } = re;
const localAlias = local;
const { localValue } = local;
const bigArray = [n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n];
const exactArray = [n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n];
const nested = [[makeVoid()]];
function mkEither(flag: boolean) {
  return flag ? () => {} : makeVoid();
}
const either = mkEither(false);
function id(f: VoidFunction) {
  return f;
}
const identity = id(makeVoid());
const localIdentity = id(n);
const box = { mk() { return makeVoid(); }, own() { return n; } };
const fromMethod = box.mk();
const fromOwn = box.own();
class Holder {
  f = makeVoid();
  g = n;
}
const holder = new Holder();
const d1 = makeVoid();
const d2 = d1;
const d3 = d2;
const d4 = d3;
const d5 = d4;
const d6 = d5;
const d7 = d6;
const d8 = d7;
const d9 = d8;
const d10 = d9;
const d11 = d10;
const d12 = d11;
const d13 = d12;
const d14 = d13;
const d15 = d14;
const d16 = d15;
const d17 = d16;
const d18 = d17;
const d19 = d18;
const d20 = d19;
const d21 = d20;
const d22 = d21;
const d23 = d22;
const d24 = d23;
const d25 = d24;
const d26 = d25;
const d27 = d26;
const d28 = d27;
const d29 = d28;
const d30 = d29;
const d31 = d30;
const d32 = d31;
const d33 = d32;
const d34 = d33;
const d35 = d34;
const d36 = d35;
const d37 = d36;
const d38 = d37;
const d39 = d38;
const d40 = d39;
const d41 = d40;
const d42 = d41;
const d43 = d42;
const d44 = d43;
const d45 = d44;
const d46 = d45;
const d47 = d46;
const d48 = d47;
const d49 = d48;
const d50 = d49;
const d51 = d50;
const d52 = d51;
const d53 = d52;
const d54 = d53;
const d55 = d54;
const d56 = d55;
const d57 = d56;
const d58 = d57;
const d59 = d58;
const d60 = d59;
const s1 = makeVoid();
const s2 = s1;
const s3 = s2;
const s4 = s3;

// B1: a package declares a global (or a constructor) the library declares too, and replaces it: the signature the compiler chooses for the call is the package's.
export function OverloadGlobal() {
  setTimeout(() => {}, "fz");
  return null;
}
export function ConstructorOverload() {
  new Float32Array("fz", () => {});
  return null;
}
export function WindowVoid() {
  window.fzHook();
  return null;
}
// Silent: the same calls the library's signature takes.
export function HostCalls() {
  setTimeout(() => {}, 0);
  new Float32Array(2);
  return null;
}

// B2: a `Proxy` answers for its target, so it is not a fresh host object.
export function ProxyCall() {
  new Proxy(madeMap, {}).forEach(() => {});
  return null;
}
export function ProxyStored() {
  proxied.forEach(() => {});
  return null;
}
// Silent: a Proxy of the project's own object.
export function ProxyOfProject() {
  plainProxy.run();
  return null;
}

// B3: a project module's namespace reached through an alias, a destructuring slot, a nested `export * as`, or a member the module made.
export function NamespaceAlias() {
  [() => {}].forEach(nsAlias.reRun);
  return null;
}
export function NamespaceDestructured() {
  [() => {}].forEach(reRun);
  return null;
}
export function NamespaceNested() {
  [() => {}].forEach(outer.ns.reRun);
  return null;
}
export function NamespaceMade() {
  re.stored();
  return null;
}
// Silent: the same shapes over a module of the project's own functions.
export function NamespaceLocal() {
  [() => {}].forEach(localAlias.localFn);
  [() => {}].forEach(localValue);
  [() => {}].forEach(local.localFn);
  localFn();
  return null;
}

// B4: JavaScript the program does not analyze, through a barrel or behind a query.
export function ShimBarrel() {
  viaBarrel(() => {});
  return null;
}
export function ShimQuery() {
  viaQuery(() => {});
  return null;
}

// S1: a value a package made that the walk follows through a conditional, a parameter, a method, a field, a nested array, or a standard-library function that hands it on.
export function Conditional() {
  either();
  return null;
}
export function Parameter() {
  identity();
  return null;
}
export function Method() {
  fromMethod();
  return null;
}
export function Field() {
  holder.f();
  return null;
}
export function NestedArray() {
  nested[0][0]();
  return null;
}
export function AssignedStd() {
  Object.assign({}, listener).handleEvent(new Event("x"));
  return null;
}
export function ArrayFrom() {
  Array.from([runStep])[0](() => {});
  return null;
}
// Silent: the project's own functions through the same shapes.
export function ProjectValues() {
  localIdentity();
  fromOwn();
  holder.g();
  exactArray[255]();
  return null;
}

// S1: a limit is a site, never a silence: an array of more than 256 elements, a chain of more than 48 hops.
export function ArrayLimit() {
  bigArray[256]();
  return null;
}
export function DepthLimit() {
  d60();
  return null;
}
export function DepthWithin() {
  s4();
  return null;
}
