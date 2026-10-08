import { createTicker } from "reactive-package";

// The returned start reads the ticker's signal: in the component body, untracked.
export function StartInBody() {
  const [running, start] = createTicker();
  start();
  return <div>{String(running())}</div>;
}
// Handed to a handler, start runs later, outside the render.
export function StartAsHandler() {
  const [running, start] = createTicker();
  return <button onClick={start}>{String(running())}</button>;
}
// Called inside a handler closure: the read happens in the handler.
export function StartInHandlerClosure() {
  const [running, start] = createTicker();
  return <button onClick={() => start()}>{String(running())}</button>;
}
// A `let` binding may be reassigned: the call is not bound to the effects.
export function LetBinding() {
  let [running, start] = createTicker();
  start();
  return <div>{String(running())}</div>;
}
function keep(value: unknown) {
  return value;
}
// Passed on, start may be called anywhere.
export function EscapedStart() {
  const [running, start] = createTicker();
  keep(start);
  return <div>{String(running())}</div>;
}
// Reached through the tuple rather than destructured.
export function ThroughTheTuple() {
  const ticker = createTicker();
  ticker[1]();
  return <div>{String(ticker[0]())}</div>;
}

// ADR 0250: escapes the binder sees but TypeFacts may emit no entity for, and
// a wrapped call that does not instantiate the member's effects. Each is an
// obligation, never silently clean.
function keepValue(value: unknown) {
  return value;
}
export function TupleInArray() {
  const ticker = createTicker();
  keepValue([ticker]);
  return <div />;
}
export function TupleInShorthand() {
  const ticker = createTicker();
  keepValue({ ticker });
  return <div />;
}
export function WrappedMemberCall() {
  const [running, start] = createTicker();
  (start as () => void)();
  return <div>{String(running())}</div>;
}
