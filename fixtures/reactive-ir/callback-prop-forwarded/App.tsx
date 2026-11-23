import { createContext, createMemo, onSettled, useContext } from "solid-js";

// ADR 1190 (slice 4): a callback prop forwarded through a component (`run={props.run}`, or `{...props}`) is entered by the
// last child's calls.

function SettledKid(props: { run: () => void }) {
  onSettled(() => {
    props.run();
  });
  return <p>x</p>;
}
function Forward(props: { run: () => void }) {
  return <SettledKid run={props.run} />;
}
// Forwarded once, called in onSettled: PRIMITIVE_IN_FORBIDDEN_SCOPE. SC3001 violation at the call.
export function ForwardedSettled() {
  return (
    <Forward
      run={() => {
        createMemo(() => 1);
      }}
    />
  );
}

function BodyKid(props: { run: () => void }) {
  props.run();
  return <p>x</p>;
}
function ForwardBody(props: { run: () => void }) {
  return <BodyKid run={props.run} />;
}
// Forwarded once, called in the child's body: clean.
export function ForwardedBody() {
  return (
    <ForwardBody
      run={() => {
        createMemo(() => 1);
      }}
    />
  );
}

function SpreadSettledKid(props: { run: () => void }) {
  onSettled(() => {
    props.run();
  });
  return <p>x</p>;
}
function Spread(props: { run: () => void }) {
  return <SpreadSettledKid {...props} />;
}
// Spread on, called in onSettled: SC3001 violation at the call.
export function SpreadSettled() {
  return (
    <Spread
      run={() => {
        createMemo(() => 1);
      }}
    />
  );
}

function SpreadBodyKid(props: { run: () => void }) {
  props.run();
  return <p>x</p>;
}
function SpreadBody(props: { run: () => void }) {
  return <SpreadBodyKid {...props} />;
}
// Spread on, called in the child's body: clean.
export function SpreadedBody() {
  return (
    <SpreadBody
      run={() => {
        createMemo(() => 1);
      }}
    />
  );
}

function OverrideKid(props: { run: () => void }) {
  onSettled(() => {
    props.run();
  });
  return <p>x</p>;
}
function Override(props: { run: () => void }) {
  return <OverrideKid {...props} run={() => {}} />;
}
// The spread's `run` is overridden by a later attribute: the literal never reaches the child. Rows kept.
export function SpreadOverridden() {
  return (
    <Override
      run={() => {
        createMemo(() => 1);
      }}
    />
  );
}

function ButtonForward(props: { run: () => void }) {
  return <button onClick={props.run}>go</button>;
}
// Forwarded to an intrinsic element's handler: not this proof. Rows kept.
export function ForwardedToButton() {
  return (
    <ButtonForward
      run={() => {
        createMemo(() => 1);
      }}
    />
  );
}

// A default-less context never provided, read by a literal forwarded once and called in the last child's body, under its owner:
// CONTEXT_NOT_FOUND (owner-callback-prop's `Forwarded`). SC4002 violation.
const Missing = createContext<number>();
function ContextKid(props: { run: () => void }) {
  props.run();
  return <p>x</p>;
}
function ContextForward(props: { run: () => void }) {
  return <ContextKid run={props.run} />;
}
export function ForwardedContext() {
  return <ContextForward run={() => useContext(Missing)} />;
}
