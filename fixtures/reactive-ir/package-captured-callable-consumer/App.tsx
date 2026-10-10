import { createOptimistic } from "solid-js";
import { createTicker, createPanel, createResource, createResult, createUnknownResult } from "reactive-package";

export function CapturedReadInBody() {
  const [read] = createOptimistic(0);
  const f = createTicker(read);
  f(); // SC1001, proven violation: invokes the captured accessor here.
  return <div />;
}
export function CapturedReadInJSX() {
  const [read] = createOptimistic(0);
  const f = createTicker(read);
  return <div data-value={f()} />; // clean: tracked invocation.
}
export function CapturedReadInHandler() {
  const [read] = createOptimistic(0);
  const f = createTicker(read);
  return <button onClick={() => f()} />; // clean: exact call in event body.
}
export function DeferredRead() {
  const [read] = createOptimistic(0);
  const f = createPanel(read);
  f(); // clean: capture runs later, outside the component's strict-read window.
  return <div />;
}
export function InvocationArgumentIsSeparate() {
  const [read] = createOptimistic(0);
  const f = createTicker(read);
  f(() => 0); // SC1001: this argument cannot replace the captured read.
  return <div />;
}
export function TwoCapturedInstances() {
  const [read] = createOptimistic(0);
  const reads = createTicker(() => read());
  const plain = createTicker(() => 0);
  reads(); // SC1001: only this instance invokes the captured read.
  plain(); // clean: the other factory instance cannot acquire that capture.
  return <div />;
}
export function WrappedReturnedInitializer() {
  const [read] = createOptimistic(0);
  const f = createTicker(read) as () => void; // SC9012: wrapped instance.
  f(); // SC9012; no wrapper-derived proven violation.
  return <div />;
}
export function Shadowed() {
  const callback = () => {};
  const f = createTicker(callback);
  { const callback = () => {}; callback(); }
  f(); // clean: shadowed declaration does not invalidate capture identity.
  return <div />;
}
function keep(value: unknown) { return value; }
export function ReturnedEscapes() {
  const callback = () => {};
  const f = createTicker(callback);
  const array = [f]; // SC9012.
  const object = { f }; // SC9012, binder shorthand.
  const alias = f; // SC9012.
  keep(f); // SC9012.
  (f as () => void)(); // SC9012.
  alias(); // no alias-derived proof.
  return <div data-array={array} data-object={object} data-callback={f} />;
}
export function ReturnedAsHandlerValue() {
  const f = createTicker(() => {});
  return <button onClick={f} />; // SC9012: captured callable escapes to on*.
}
export function ReturnedByWrapper() {
  const f = createTicker(() => {});
  return f; // SC9012.
}
export const exported = createTicker(() => {}); // SC9012.
const laterExport = createTicker(() => {});
export { laterExport }; // SC9012.
export function CapturedEscapes() {
  const callback = () => {};
  const f = createTicker(callback);
  const array = [callback]; // SC9012.
  const object = { callback }; // SC9012.
  const alias = callback; // SC9012.
  keep(callback); // SC9012.
  (callback as () => void)(); // SC9012.
  f(); // SC9012: capture binding withheld.
  return <div data-array={array} data-object={object} data-callback={callback} data-alias={alias} />;
}
export function CapturedAsHandlerValue() {
  const callback = () => {};
  const f = createTicker(callback);
  f(); // SC9012: capture escapes into JSX below.
  return <button onClick={callback} />; // SC9012.
}
export function ReturnCapturedValue() {
  const callback = () => {};
  createTicker(callback);
  return callback; // SC9012.
}
export const capturedExport = () => {};
const exportedCapture = createTicker(capturedExport); // SC9012.
export { exportedCapture };
export function CaptureAlias() {
  const callback = () => {};
  const alias = callback;
  const f = createTicker(alias); // SC9012: capture source is an alias.
  f(); // SC9012.
  return <div />;
}
export function CaptureWrapper() {
  const callback = () => {};
  const f = createTicker(callback as () => void); // SC9012.
  f(); // SC9012.
  return <div />;
}
export function MutableCapture() {
  let callback = () => {};
  const f = createTicker(callback); // SC9012.
  callback = () => {};
  f(); // SC9012.
  return <div />;
}
declare function opaqueFactory(): () => void;
export function UnknownCapture() {
  const callback = opaqueFactory();
  const f = createTicker(callback);
  f(); // SC9012: exact binding does not prove an implementation.
  return <div />;
}
export function MutableReturned() {
  let f = createTicker(() => {});
  f(); // SC9012.
  return <div />;
}
export function CaptureInputsUnbound() {
  const callback = (value = () => {}) => value();
  const f = createTicker(callback); // SC9012: invocation inputs are unbound.
  f(() => {}); // SC9012; no guessed forwarding into the retained callback.
  return <div />;
}
export function NestedLiteralIsNotTheCapture() {
  const [read] = createOptimistic(0);
  const f = createTicker(() => { const nested = () => read(); void nested; });
  f(); // clean: exact outer arrow does not execute the nested read.
  return <div />;
}
export function Discard() {
  createTicker(() => {}); // clean: no invocation and no escape.
  return <div />;
}
export function ResourceCaptureRead() {
  const f = createResource();
  f(); // SC1001: exact factory resource, read by the returned graph.
  return <div />;
}
export function ResourceCaptureTracked() {
  const f = createResource();
  return <div data-value={f()} />; // clean.
}
export function OperationResultCaptureRead() {
  const f = createResult();
  f(); // SC1001: exact stated accessor result names the same resource.
  return <div />;
}
export function OperationResultCaptureTracked() {
  const f = createResult();
  return <div data-value={f()} />; // clean.
}
export function UnknownOperationResultCapture() {
  const f = createUnknownResult();
  f(); // SC9012: the factory result's accessor identity is not established.
  return <div />;
}
