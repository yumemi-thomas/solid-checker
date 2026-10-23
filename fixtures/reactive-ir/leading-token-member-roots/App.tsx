import { createSignal, createStore } from "solid-js";

function noop() {}
const fixed = { onSave: noop };
const getFixed = () => fixed;
const never = () => false;
const fixedReady = { ready: never };
const getFixedReady = () => fixedReady;

type HandlerProps = { onSave: () => void; handlers: () => { onSave: () => void } };

// VIOLATION (SC1007): the listener is the signal-backed `props.onSave`.
function DirectHandler(props: HandlerProps) {
  return <button onClick={props.onSave}>x</button>;
}
// CLEAN of SC1007: the listener is the `onSave` of what the static prop
// `handlers` returns; leading token `props` and leaf `onSave` read the
// reactive prop `onSave` (before: SC1007).
function CallRootedHandler(props: HandlerProps) {
  return <button onClick={props.handlers().onSave}>x</button>;
}

type ReadyProps = { ready: () => boolean; make: () => { ready: () => boolean } };

// VIOLATION (SC8015): `props.ready()` calls the accessor the caller passes.
function DirectCondition(props: ReadyProps) {
  return <div>{props.ready() && <span title="a" />}</div>;
}
// CLEAN: `props.make().ready()` calls `never`, not the prop `ready`
// (before: SC8015 through the leading token `props`).
function CallRootedCondition(props: ReadyProps) {
  return <div>{props.make().ready() && <span title="b" />}</div>;
}

export function App() {
  const [store] = createStore({ save: noop });
  const [on] = createSignal(true);
  return (
    <div>
      <DirectHandler onSave={store.save} handlers={getFixed} />
      <CallRootedHandler onSave={store.save} handlers={getFixed} />
      <DirectCondition ready={on} make={getFixedReady} />
      <CallRootedCondition ready={on} make={getFixedReady} />
    </div>
  );
}
