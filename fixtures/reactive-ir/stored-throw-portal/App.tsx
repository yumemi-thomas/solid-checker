import { Errored, createContext, useContext } from "solid-js";
import { Portal } from "@solidjs/web";
import * as Web from "@solidjs/web";

// Default-less and never provided: a read under an owner throws ContextNotFoundError, a plain throw with no diagnostic (ADR 0893).
const Bare = createContext<number>();

// ADR 1043: `Portal` reads `children` only in its own memo, under an owner below the element (`Dialect::component_entry`, ADR 0992), so a component in
// its children renders in place: the throw in its body ends where the `<Portal>` element's does, and the render around it reports it.

// Violations (SC4002).
function InPortal() {
  useContext(Bare);
  return <p>x</p>;
}
export function PortalChild() {
  return <div><Portal><InPortal /></Portal></div>;
}
function InNamespacePortal() {
  useContext(Bare);
  return <p>x</p>;
}
export function NamespacePortalChild() {
  return <div><Web.Portal><InNamespacePortal /></Web.Portal></div>;
}

// A call written in the children is evaluated by the Portal's memo: the component it makes renders there too.
function InCalledChild() {
  useContext(Bare);
  return <p>x</p>;
}
export function CalledChild() {
  const make = () => <InCalledChild />;
  return <div><Portal>{make()}</Portal></div>;
}

// Obligations (uncertifiable): a boundary above the Portal takes the throw, or the Portal does not render the component in place.
function UnderBoundary() {
  useContext(Bare);
  return <p>x</p>;
}
export function BoundaryAbovePortal() {
  return (
    <Errored fallback={(e) => <p>{String(e)}</p>}>
      <Portal><UnderBoundary /></Portal>
    </Errored>
  );
}
function InSpread() {
  useContext(Bare);
  return <p>x</p>;
}
export function SpreadPortal(props: { mount?: Element }) {
  return <div><Portal {...props}><InSpread /></Portal></div>;
}
// `mount` is no slot the Portal renders in place (it reads it again in its render effect's apply).
function InMount() {
  useContext(Bare);
  return document.body;
}
export function MountProp() {
  return <div><Portal mount={InMount()}><p>x</p></Portal></div>;
}
