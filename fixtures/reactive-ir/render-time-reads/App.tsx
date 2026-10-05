import { createSignal } from "solid-js";

const [count] = createSignal(1);

// Two levels of plain calls, each written in the previous function's body.
function inner() {
  return count();
}
function outer() {
  return inner();
}
// The same callee, entered only from a closure that is never called here.
function deferred() {
  return () => inner();
}

// A default runs during the call when the call omits its argument.
function withDefault(value = count()) {
  return value;
}
// A default that only builds a function reads nothing.
function lazyDefault(read = () => count()) {
  return read;
}

// A nested function's default lies in the hook's body but runs only when that
// function is called, which this hook never does.
function useFinisher() {
  const finish = (value = count()) => value;
  return finish;
}

// Calls its callback prop in its own body, while it renders.
function Render(props: { label: () => number }) {
  const text = props.label();
  return <div>{text}</div>;
}
// Calls it only in JSX, which tracks.
function RenderLive(props: { label: () => number }) {
  return <div>{props.label()}</div>;
}

export function App(rest: { label?: () => number }) {
  const viaChain = outer();
  const omitted = withDefault();
  const passed = withDefault(2);
  const later = deferred();
  const lazy = lazyDefault();
  const finisher = useFinisher();
  return (
    <main>
      {viaChain}
      {omitted}
      {passed}
      {String(later)}
      {String(lazy)}
      {String(finisher)}
      <Render label={() => count()} />
      <RenderLive label={() => count()} />
      <Render label={() => count()} {...rest} />
    </main>
  );
}
