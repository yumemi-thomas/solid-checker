import { createSignal } from "solid-js";

function Handler(props: { onClick: () => void }) {
  return <button onClick={() => props.onClick()}>run</button>;
}

// Creating the callback does not prove invocation during component rendering.
export function ForwardedHandler() {
  const [n] = createSignal(0);
  return <Handler onClick={() => console.log(n())} />;
}

export function WrappedForwardedHandler() {
  const [n] = createSignal(0);
  return <Handler onClick={(() => console.log(n())) as () => void} />;
}

// This read actually runs in the component body, before creating a callback.
export function EagerHandlerFactory() {
  const [n] = createSignal(0);
  const snapshot = n();
  return <Handler onClick={() => console.log(snapshot)} />;
}

// A nested value is not proven readonly merely because props is readonly.
export function NestedMutableValue(props: { state: { count: number } }) {
  return <button onClick={() => { props.state.count++; }}>{props.state.count}</button>;
}

export function WrappedNestedValue(props: { state: { count: number } }) {
  return <button onClick={() => { (props.state as { count: number }).count = 2; }}>{props.state.count}</button>;
}

// The direct props container remains readonly at runtime, with mutable typings.
export function DirectPropsWrite(props: { count: number }) {
  return <button onClick={() => { props.count = 2; }}>{props.count}</button>;
}

export function PropsControls() {
  const state = { count: 0 };
  return <span><NestedMutableValue state={state} /><WrappedNestedValue state={state} />
    <DirectPropsWrite count={0} /><ForwardedHandler /><WrappedForwardedHandler /><EagerHandlerFactory /></span>;
}
