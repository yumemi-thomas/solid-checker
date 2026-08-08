import {
  createSignal,
  type Component,
} from "solid-js";

export const Ternary: Component<{ big: boolean }> = props =>
  props.big ? <div>Big</div> : <div>Small</div>;

export const EarlyReturn: Component<{ error: boolean }> = props => {
  if (props.error) return <div>Error</div>;
  return <div>Ready</div>;
};

export const SignalGuard: Component = () => {
  const [failed] = createSignal(false);
  if (failed()) return <div>Failed</div>;
  return <div>Ready</div>;
};

// Upstream flags a conditional return even when the condition is a plain
// constant: the component body runs once either way.
const STATIC = false;

export const StaticGuard: Component = () =>
  STATIC ? <div>Debug</div> : <div>Ready</div>;

export const StaticEarlyReturn: Component<{ ready: boolean }> = props => {
  if (STATIC) return <div>Debug</div>;
  const label = props.ready ? "Ready" : "Waiting";
  return <div>{label}</div>;
};
