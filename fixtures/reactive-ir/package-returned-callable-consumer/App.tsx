import { createTicker, createPanel } from "reactive-package";

export function CallInBody() {
  const f = createTicker();
  f(); // SC1001 violation: the stated graph reads in the caller context.
  return <div />;
}
export function CallInHandler() {
  const f = createTicker();
  return <button onClick={() => f()} />; // clean
}
export function DirectEventValue() {
  const f = createTicker();
  return <button onClick={f} />; // ADR 0234 exemption
}
export function ClearInBody() {
  const f = createTicker();
  f.clear(); // SC1001 violation from clear's separate graph.
  return <div />;
}
export function ClearInHandler() {
  const f = createTicker();
  return <button onClick={() => f.clear()} />; // clean
}
export function OpaqueMember() {
  const f = createTicker();
  f.opaque(); // reactive-dispatch-unresolved, uncertifiable
  f.unknown(); // reactive-dispatch-unresolved, uncertifiable
  return <div />;
}
export function OpaqueInHandler() {
  const f = createTicker();
  return <button onClick={() => f.opaque()} />; // ADR 0234 exemption
}
export function NoGraph() {
  const f = createPanel();
  f(); // reactive-dispatch-unresolved, uncertifiable
  return <div />;
}
function keep(value: unknown) { return value; }
export function Escape() {
  const f = createTicker();
  keep(f); // obligation: passed on
  return <div />;
}
export function Alias() {
  const f = createTicker();
  const other = f; // obligation: aliased
  other();
  return <div />;
}
export function Store() {
  const f = createTicker();
  const stored = [f]; // obligation: stored
  return <div data-value={stored} />;
}
export function ReturnEscape() {
  const f = createTicker();
  return f; // obligation: returned
}
export const exported = createTicker(); // obligation even without references
const exportedLater = createTicker();
export { exportedLater }; // obligation: export specifier
export function JSXAttribute() {
  const f = createTicker();
  return <div data-function={f} />; // obligation: JSX attribute
}
export function Discard() {
  createTicker(); // clean: no returned invocation or escape
  return <div />;
}
export function LetBinding() {
  let f = createTicker(); // obligation: mutable binding
  f(); // obligation: graph was not bound
  return <div />;
}
export function MutatedMember() {
  const f = createTicker();
  f.clear = () => {};
  f.clear(); // obligation, never the old graph's proven violation
  return <div />;
}
export function WrappedAndShadowed() {
  const f = (createTicker() as ReturnType<typeof createTicker>);
  (f as () => void)(); // uncertifiable: a wrapped callee does not instantiate the graph
  { const f = () => {}; f(); } // clean: another exact binding
  return <div />;
}
export function TwoInstances() {
  const first = createTicker();
  const second = createTicker();
  first.clear(); // SC1001: first factory instance
  return <button onClick={() => second.clear()} />; // clean
}

export function ReactiveMemberInBody() {
  const f = createTicker();
  f.value(); // SC1001: exact returned reactive accessor member
  return <div />;
}
export function ReactiveMemberInHandler() {
  const f = createTicker();
  return <button onClick={() => f.value()} />; // clean
}
export function MutatedReactiveMember() {
  const f = createTicker();
  f.value = () => false;
  f.value(); // obligation, no inherited reactive property symbol
  return <div />;
}
