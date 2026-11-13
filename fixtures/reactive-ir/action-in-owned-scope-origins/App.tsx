import { createMemo } from "solid-js";
import { listener, makeHandle, makeVoid, typedStep } from "fz-typed";
import { noop } from "fz-types";
import kernel from "fz-obj";
import { runStep } from "fz-step";
import { typedPure } from "fz-pure";
import "fz-augment";
import { reRun } from "./re";
import { vendorStep } from "./vendor/step";

// Values a package made at module scope, where no owner is active.
const stored = makeVoid();
const handle = makeHandle();
const alias = runStep;
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
