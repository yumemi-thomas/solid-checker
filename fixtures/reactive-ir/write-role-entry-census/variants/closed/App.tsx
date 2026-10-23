import { createSignal } from "solid-js";

// Hands its `onPress` prop to the button as its click handler.
function Button(props: { onPress: () => void }) {
  return <button onClick={props.onPress} />;
}

// Calls its `run` prop in its own body, under the component's owner.
function RunsInBody(props: { run: () => void }) {
  props.run();
  return <p />;
}

// Clean: `reset` escapes as a prop, and ADR 0319's walk follows it to the
// button's click handler, the only place it runs.
export function PropHandlerOnly() {
  const [n, setN] = createSignal(0);
  const reset = () => setN(0);
  return (
    <>
      <button onClick={() => reset()}>{n()}</button>
      <Button onPress={reset} />
    </>
  );
}

// Not clean: the walk finds `RunsInBody`'s call in its body, which is no event
// delivery, so the escape is not followed to a role.
export function PropCalledInBody() {
  const [n, setN] = createSignal(0);
  const reset = () => setN(0);
  return (
    <>
      <button onClick={() => reset()}>{n()}</button>
      <RunsInBody run={reset} />
    </>
  );
}

const [total, setTotal] = createSignal(0);

// Clean in this closed program: the export's only entry is the click handler's
// call.
export const resetTotal = () => setTotal(0);

export function ExportedHelper() {
  return <button onClick={() => resetTotal()}>{total()}</button>;
}

// Clean: `run` reaches only `Button`'s click handler, so its call of `reset`
// is an event delivery, as is the inline handler's; ADR 0327's walk proves
// every invocation of `reset` an event.
export function ForwardedThroughHelper() {
  const [n, setN] = createSignal(0);
  const reset = () => {
    setN(1);
  };
  const run = () => reset();
  return (
    <>
      <Button onPress={run} />
      <button onClick={() => reset()}>{n()}</button>
    </>
  );
}
