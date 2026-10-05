import { merge, omit } from "solid-js";

// Components that receive a callback prop. Each one's uses of `props.onPress`
// decide whether a function literal passed to it can run while its caller's
// body renders.

type Props = { onPress?: (event: unknown) => void; label?: string };

// Forwarded as the handler of an intrinsic element's event attribute.
export function Direct(props: Props) {
  return <button onClick={props.onPress}>{props.label}</button>;
}

// Invoked from inside an intrinsic element's event handler.
export function Wrapped(props: Props) {
  return <button onClick={(event) => props.onPress?.(event)}>{props.label}</button>;
}

// Forwarded to another component that forwards it to an event.
export function Chained(props: Props) {
  return <Direct onPress={props.onPress} label={props.label} />;
}

// Invoked while the component body runs.
export function CallsDuringRender(props: Props) {
  props.onPress?.(undefined);
  return <button>{props.label}</button>;
}

// Spread onto an intrinsic element: the runtime attaches an `on…` property as
// an event listener.
export function Spreads(props: Props) {
  return <button {...props} />;
}

// Read through a merge view inside an event handler.
export function Merged(props: Props) {
  const p = merge({ label: "default" }, props);
  return <button onClick={(event) => p.onPress?.(event)}>{p.label}</button>;
}

// Spread through an omit view.
export function OmitSpread(props: Props) {
  return <button {...omit(props, "label")}>{props.label}</button>;
}

// Spread onto a component that calls the prop while rendering.
export function SpreadsToCaller(props: Props) {
  return <CallsDuringRender {...props} />;
}

// A merge view handed to an unknown helper.
export function MergeEscapes(props: Props) {
  const p = merge(props);
  keep(p);
  return <button>{props.label}</button>;
}
function keep(value: unknown) {
  void value;
}

// A spread carries a prop whose name is not an event's.
export function SpreadsRender(props: { render?: () => void }) {
  return <div {...props} />;
}

// Kept in a timer: when it runs is the host's business.
export function Keeps(props: Props) {
  setTimeout(() => props.onPress?.(undefined), 0);
  return <button>{props.label}</button>;
}

// One use is an event; another runs while rendering.
export function Mixed(props: Props) {
  return (
    <button onClick={props.onPress}>
      {String(props.onPress?.(undefined))}
    </button>
  );
}

// Forwards to itself: a cycle proves nothing.
export function Loop(props: Props): JSX.Element {
  return <Loop onPress={props.onPress} />;
}
