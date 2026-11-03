import { createContext, onCleanup, onSettled, useContext } from "solid-js";
import * as S from "solid-js";
import { Remote } from "./remote";

// Default-less and never provided: a read under an owner throws ContextNotFoundError.
const Ctx = createContext<number>();
// Default-less and provided above `ProvidedParent`'s child.
const ProvidedCtx = createContext<number>();

type RunProps = { run: () => void };

// Positive (SC4002): the child calls the prop in its body, under its own owner.
export function BodyCall() {
  return <Kid run={() => useContext(Ctx)} />;
}
function Kid(props: RunProps) {
  props.run();
  return <p>kid</p>;
}

// Positive: a destructured prop called in the body.
export function DestructuredCall() {
  return <KidDestructured run={() => useContext(Ctx)} />;
}
function KidDestructured({ run }: RunProps) {
  run();
  return <p>kid</p>;
}

// Positive: the child calls the prop in its JSX (a render expression, owned).
export function JsxCall() {
  return <KidJsx run={() => S.useContext(Ctx)} />;
}
function KidJsx(props: RunProps) {
  return <p>{(props.run(), "x")}</p>;
}

// Negative: the provider supplies the context above the child.
export function ProvidedParent() {
  return (
    <ProvidedCtx value={1}>
      <Kid run={() => useContext(ProvidedCtx)} />
    </ProvidedCtx>
  );
}

// Negative: a cleanup in a prop the child calls in its body is owned (NO_OWNER_CLEANUP's legal twin).
export function OwnedCleanup() {
  return <Kid run={() => onCleanup(() => {})} />;
}

// Negative (obligation): a prop never called runs nothing; nothing proves a run.
export function NeverCalled() {
  return <KidNever run={() => useContext(Ctx)} />;
}
function KidNever(props: { run?: () => void }) {
  return <p>{props.run ? "set" : "unset"}</p>;
}

// Negative (obligation): a prop forwarded to another component is not followed.
export function Forwarded() {
  return <Forwarder run={() => useContext(Ctx)} />;
}
function Forwarder(props: RunProps) {
  return <KidForwardTarget run={props.run} />;
}
function KidForwardTarget(props: RunProps) {
  props.run();
  return <p>kid</p>;
}

// Negative (obligation): the child copies the prop and calls the copy from a timer.
export function Copied() {
  return <KidTimer run={() => useContext(Ctx)} />;
}
function KidTimer(props: RunProps) {
  const r = props.run;
  setTimeout(r);
  return <p>kid</p>;
}

// Negative (obligation): a call inside the child's `onSettled` callback runs under that callback's owner,
// which this proof does not follow.
export function SettledCall() {
  return <KidSettled run={() => useContext(Ctx)} />;
}
function KidSettled(props: RunProps) {
  onSettled(() => props.run());
  return <p>kid</p>;
}

// Negative (obligation): a child in another file is not followed.
export function OtherFile() {
  return <Remote run={() => useContext(Ctx)} />;
}
