import { createContext, createMemo } from "solid-js";
import { SharedContext } from "./ctx";

// ADR 1123, the review's F2 and F3: the children arm (a dialect provider reads its written children in place) holds only for a
// provider whose tag is this file's unwritten, unexported `const` of one name that `createContext` initializes, and whose element
// is placed directly in owned JSX. Every other case keeps the child creation's rows (SC3001, SC6002: uncertifiable), as any
// component's children. Each child creates a memo, so where its creation runs is a question.

const C = createContext<number>();

// A component an unread declaration supplies: it may run what it is given anywhere.
declare const Ext: (props: { value: number; children: JSX.Element }) => JSX.Element;
declare function keep(value: unknown): void;

// Proved (silent): the provider is the component's returned value, and a child of an intrinsic element.
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
