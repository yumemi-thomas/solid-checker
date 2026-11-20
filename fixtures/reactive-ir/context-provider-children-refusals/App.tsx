import { createContext, createMemo } from "solid-js";
import { SharedContext } from "./ctx";

// ADR 1123 withdrew the provider-children arm (a dialect provider reads its written children in place, so a child runs where the provider
// element is created). rc.21's provider returns `children(() => props.children)`, a memo its READER runs: the children run where the
// provider's value is read, which no test of where the provider element is written can say (the review's F2, F3 and N1). So every case
// keeps its child creation's rows (SC3001, SC6002: uncertifiable), as any component's children do, `ReturnedApp` and `NestedApp` included
// (the arm certified those two). These cases are the redo's acceptance set (docs/open-work.md): a transitive placement may certify the
// first two only; each other case must keep its rows. Each child creates a memo, so where its creation runs is a question.
// ADR 1147 (arm 1): a creation is an owner code, which the provider's children memo answers wherever it is read, so the arm ends the child rows of
// every case but the F2 ones (no exact provider); the stack codes (a flush) are the redo's question here (README).

const C = createContext<number>();

// A component an unread declaration supplies: it may run what it is given anywhere.
declare const Ext: (props: { value: number; children: JSX.Element }) => JSX.Element;
declare function keep(value: unknown): void;

// The provider is the component's returned value, and a child of an intrinsic element (the withdrawn arm certified both; a redo may).
function Returned() {
  createMemo(() => 1);
  return <i />;
}
export function ReturnedApp() {
  return (
    <C value={1}>
      <Returned />
    </C>
  );
}
function Nested() {
  createMemo(() => 1);
  return <i />;
}
export function NestedApp() {
  return (
    <div>
      <C value={1}>
        <Nested />
      </C>
    </div>
  );
}

// F2: a written `let` context.
let W = createContext<number>();
W = createContext<number>();
function Written() {
  createMemo(() => 1);
  return <i />;
}
export function WrittenApp() {
  return (
    <W value={1}>
      <Written />
    </W>
  );
}

// F2: a pattern's name the context's `defaultValue` initializes: the tag is the unread component, not a provider.
const { defaultValue: Picked = Ext } = createContext(Ext);
function Pattern() {
  createMemo(() => 1);
  return <i />;
}
export function PatternApp() {
  return (
    <Picked value={1}>
      <Pattern />
    </Picked>
  );
}

// F2: a context another module declares.
function Imported() {
  createMemo(() => 1);
  return <i />;
}
export function ImportedApp() {
  return (
    <SharedContext value={1}>
      <Imported />
    </SharedContext>
  );
}

// F3: the provider element stored, then placed.
function Stored() {
  createMemo(() => 1);
  return <i />;
}
export function StoredApp() {
  const element = (
    <C value={1}>
      <Stored />
    </C>
  );
  return <div>{element}</div>;
}

// F3: the provider element passed to a call nothing here reads.
function Passed() {
  createMemo(() => 1);
  return <i />;
}
export function PassedApp() {
  keep(
    <C value={1}>
      <Passed />
    </C>
  );
  return <i />;
}

// F3: returned by a helper a call (not a tag) runs, its value placed by the caller.
function Helped() {
  createMemo(() => 1);
  return <i />;
}
function makeProvider() {
  return (
    <C value={1}>
      <Helped />
    </C>
  );
}
export function HelpedApp() {
  const made = makeProvider();
  return <div>{made}</div>;
}

// F3: a child of the unread component: handed with it.
function Handed() {
  createMemo(() => 1);
  return <i />;
}
export function HandedApp() {
  return (
    <Ext value={1}>
      <C value={1}>
        <Handed />
      </C>
    </Ext>
  );
}

// N1 (review round 2): the provider is the whole return of a component only tags render, and that component's own element is stored and
// handed to the unread component: the children memo travels with the element, so where the element goes decides where the children run.
function ReturnedStoredChild() {
  createMemo(() => 1);
  return <i />;
}
function ReturnedStoredInner() {
  return (
    <C value={1}>
      <ReturnedStoredChild />
    </C>
  );
}
export function ReturnedStoredApp() {
  const made = <ReturnedStoredInner />;
  return <Ext value={1}>{made}</Ext>;
}

// N1: the same element in an array handed on.
function ArrayHandedChild() {
  createMemo(() => 1);
  return <i />;
}
function ArrayHandedInner() {
  return (
    <C value={1}>
      <ArrayHandedChild />
    </C>
  );
}
export function ArrayHandedApp() {
  const list = [<ArrayHandedInner />];
  return <Ext value={1}>{list}</Ext>;
}

// N1: the provider two components down, the outer element stored and handed on.
function NestedStoredChild() {
  createMemo(() => 1);
  return <i />;
}
function NestedStoredInner() {
  return (
    <C value={1}>
      <NestedStoredChild />
    </C>
  );
}
function NestedStoredMid() {
  return <NestedStoredInner />;
}
export function NestedStoredApp() {
  const made = <NestedStoredMid />;
  return <Ext value={1}>{made}</Ext>;
}
