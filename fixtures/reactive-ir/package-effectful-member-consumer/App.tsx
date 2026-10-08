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
