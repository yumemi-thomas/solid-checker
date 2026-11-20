import { createContext, createMemo, createTrackedEffect, flush, onCleanup } from "solid-js";
import { SharedContext } from "./ctx";

// ADR 1147, arm 1 (`context-provider-children-owned`): rc.21's provider is
// `createRoot(() => { setContext(provider, props.value); return children(() => props.children); })`, so a child of an exact provider (the context `const`
// itself as the tag) runs inside the children memo's computation, whose owner is the memo, wherever and whenever the memo is read. The owner codes
// (SC3001's creation and cleanup, SC6002's creation) are answered there; the stack codes (a flush: SC3001's flush, SC2006, SC2008) are not, since the
// stack is whoever reads the memo.

const C = createContext<number>();

// A component an unread declaration supplies: it may read what it is given anywhere (in a tracked effect, say).
declare const Ext: (props: { value: number; children: JSX.Element }) => JSX.Element;

// Positive: the provider is the component's return. Kid's creation and cleanup end.
function Kid() {
  createMemo(() => 1);
  onCleanup(() => {});
  return <i />;
}
export function ReturnedApp() {
  return (
    <C value={1}>
      <Kid />
    </C>
  );
}

// Positive: the provider element stored and handed to the unread component (the review's k11 shape, owner codes only). The memo still owns the child.
function StoredKid() {
  createMemo(() => 1);
  return <i />;
}
export function StoredApp() {
  const made = (
    <C value={1}>
      <StoredKid />
    </C>
  );
  return <Ext value={1}>{made}</Ext>;
}

// Positive: through a project component that places `props.children` in the provider (textbook id-context's shape): the getter's read is a provider child.
function Provide(props: { children?: JSX.Element }) {
  return <C value={1}>{props.children}</C>;
}
function ThroughKid() {
  createMemo(() => 1);
  return <i />;
}
export function ThroughApp() {
  return (
    <Provide>
      <ThroughKid />
    </Provide>
  );
}

// Negative (F2): a written `let` holds another value at the tag.
let W = createContext<number>();
W = createContext<number>();
function WrittenKid() {
  createMemo(() => 1);
  return <i />;
}
export function WrittenApp() {
  return (
    <W value={1}>
      <WrittenKid />
    </W>
  );
}

// Negative (F2): a pattern's name the context's `defaultValue` initializes: the tag is the unread component.
const { defaultValue: Picked = Ext } = createContext(Ext);
function PatternKid() {
  createMemo(() => 1);
  return <i />;
}
export function PatternApp() {
  return (
    <Picked value={1}>
      <PatternKid />
    </Picked>
  );
}

// Negative (F2): a context another module declares (an import, not this file's `const`).
function ImportedKid() {
  createMemo(() => 1);
  return <i />;
}
export function ImportedApp() {
  return (
    <SharedContext value={1}>
      <ImportedKid />
    </SharedContext>
  );
}

// Negative (F3, the review's k11): a flush in the provider's child, the provider's value stored and handed to the unread component, which may read it
// in a tracked effect (Chrome FLUSH_IN_TRACKED_QUEUE_CALLBACK). A stack code: arm 1 never answers it.
export function FlushStoredApp() {
  const made = <C value={1}>{(flush(), 1)}</C>;
  return <Ext value={1}>{made}</Ext>;
}

// Negative (N1, the review's k9): the same one component down.
function FlushInner() {
  return <C value={1}>{(flush(), 1)}</C>;
}
export function FlushInnerApp() {
  const made = <FlushInner />;
  return <Ext value={1}>{made}</Ext>;
}

// Negative: the provider created in a tracked effect (a leaf bit on the child's entry): the child's answer still declines.
function LeafKid() {
  createMemo(() => 1);
  return <i />;
}
export function LeafApp() {
  createTrackedEffect(() => {
    void (
      <C value={1}>
        <LeafKid />
      </C>
    );
  });
  return <i />;
}

// Negative: the provider created in a handler (an unowned entry): the child's SC6002 answer still declines.
function HandlerKid() {
  createMemo(() => 1);
  return <i />;
}
export function HandlerApp() {
  return (
    <button
      onClick={() => {
        void (
          <C value={1}>
            <HandlerKid />
          </C>
        );
      }}
    />
  );
}
