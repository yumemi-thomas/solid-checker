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
