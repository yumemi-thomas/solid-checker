import { createMemo, untrack } from "solid-js";
import { listener, makeHandle, makeVoid, typedStep } from "fz-typed";
import { noop } from "fz-types";
import kernel from "fz-obj";
import { runStep } from "fz-step";
import { typedPure } from "fz-pure";
import "fz-augment";
import "fz-aug";
import { vendorStep as shimmedStep } from "./step.vendor.js";
import * as re from "./re";
import { reRun } from "./re";
import { vendorStep } from "./vendor/step";

// Values a package made at module scope, where no owner is active.
const stored = makeVoid();
const handle = makeHandle();
const alias = runStep;
const copy = Object.assign({}, kernel);
const fns = [makeVoid()];
function made() {
  return makeVoid();
}
const fromFunction = made();
function madeHandle() {
  return makeHandle();
}
const handleFromFunction = madeHandle();
const pick = () => runStep;
const { run: destructured } = kernel;

// B1: the code a callee runs is its value's origin (the import its root names), not where its signature type is declared.
export function TypedAlias() {
  typedStep(() => {}); // Runner is declared in fz-types, which is clear; the code is fz-typed's
  return null;
}
export function StdMember() {
  listener.handleEvent(new Event("x")); // EventListenerObject is the standard library's; the object is fz-typed's
  return null;
}
export function StoredVoid() {
  stored();
  return null;
}
export function StoredHandle() {
  handle.dispose();
  return null;
}
// Silent: fz-pure's code is clear whatever package declares the type of its function (fz-env-types reads `process`).
export function TypedFromHazardPackage() {
  typedPure(() => {});
  return null;
}
// Silent: the types package holds a function, and it is clear.
export function PureControl() {
  noop();
  return null;
}
// Silent: a host constructor stays the host's although fz-augment adds a signature to its type.
export function HostConstructors() {
  void new Float32Array(4);
  void Uint8Array.from([1]);
  return null;
}

// B2: a package function re-exported through a project barrel (`re.ts`), handed on as a value.
export function BarrelValue() {
  [() => {}].forEach(reRun);
  return null;
}
export function BarrelMemo() {
  const value = createMemo(reRun);
  return <p>{String(value)}</p>;
}
// A binding that only names the package function, handed on in a body.
export function AliasValue() {
  [() => {}].forEach(alias);
  return null;
}

// A name destructured from a package's object is read off what the object is.
export function DestructuredValue() {
  [() => {}].forEach(destructured);
  return null;
}
export function DestructuredCall() {
  destructured(() => {});
  return null;
}

// B3: JavaScript beside a declaration file of the project, outside the program (`allowJs` is off).
export function VendorCall() {
  vendorStep(() => {});
  return null;
}
export function VendorValue() {
  [() => {}].forEach(vendorStep);
  return null;
}

// S1: a member of a package's default-export object handed on as a value.
export function MemberValue() {
  [() => {}].forEach(kernel.run);
  return null;
}

// Silent twins: a timer callback and an event handler are legal action sites for a call.
export function TypedTimer() {
  setTimeout(() => typedStep(() => {}), 0);
  return null;
}
export function StoredHandler() {
  return <button onClick={() => handle.dispose()}>x</button>;
}
export function VendorHandler() {
  return <button onClick={() => vendorStep(() => {})}>x</button>;
}
// A function handed on stays uncertifiable even where it is handed from a handler: where the receiving call runs it is not followed (`createMemo(reRun)` in a
// handler runs it in the memo's compute).
export function BarrelValueInHandler() {
  return <button onClick={() => [() => {}].forEach(reRun)}>x</button>;
}

// R1 of the re-check: a member a package adds to a host global's type and installs is the package's code, and a host or runtime call may hand back a package value.
export function AugWindow() {
  window.fzRun(() => {}); // `interface Window { fzRun }` and `window.fzRun = …` in fz-aug
  return null;
}
export function AugConstructor() {
  Uint8Array.fzFrom(() => {}); // the same on a typed-array constructor, with code behind it
  return null;
}
export function ThroughFreeze() {
  Object.freeze(kernel).run(() => {});
  return null;
}
export function ThroughAssign() {
  copy.run(() => {});
  return null;
}
export function ThroughUntrack() {
  untrack(() => kernel).run(() => {});
  return null;
}
export function UntrackedMember(props: { k: typeof kernel }) {
  const run = untrack(() => props.k.run);
  run(() => {});
  return null;
}
export function ThroughMemo() {
  const get = createMemo(() => kernel);
  get().run(() => {});
  return null;
}
// R2: a namespace import of the barrel.
export function NamespaceOfBarrel() {
  [() => {}].forEach(re.reRun);
  return null;
}
// R3: an array element, and what a project function returns.
export function ArrayElement() {
  fns[0]();
  return null;
}
export function FunctionReturnsVoid() {
  fromFunction();
  return null;
}
export function FunctionReturnsHandle() {
  handleFromFunction.dispose();
  return null;
}
export function FunctionReturnsValue() {
  [() => {}].forEach(pick());
  return null;
}
// R4: JavaScript a wildcard ambient module (`declare module "*.vendor.js"`) stands for.
export function WildcardShim() {
  shimmedStep(() => {});
  return null;
}
