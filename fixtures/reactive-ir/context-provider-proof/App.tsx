import { createContext, useContext } from "solid-js";

// Default-less, and provided somewhere: each read needs a provider above it.
const C = createContext<number>();
let L = createContext<number>();
const o = { C };

function Read() {
  useContext(C);
  return <i />;
}

// Proven (silent): every render of the reading function is under `<C value>`,
// directly, through an intrinsic element, a hook, or a component nested inside.
function Hooked() {
  useRead();
  return <i />;
}
const useRead = () => useContext(C);
function Middle() {
  return <div><Read /></div>;
}
export function Direct() {
  return (
    <C value={1}>
      <Middle />
      <Hooked />
    </C>
  );
}

// Proven: a wrapper returning `<C value>{props.children}</C>` on its one return.
function Wrap(props: { children: JSX.Element }) {
  return <C value={1}>{props.children}</C>;
}
function ReadWrapped() {
  useContext(C);
  return <i />;
}
export function Wrapped() {
  return (
    <Wrap>
      <ReadWrapped />
    </Wrap>
  );
}

// Obligations (SC4002, `provider-not-proven-above`): a sibling subtree, a
// second render outside, a wrapper whose provider is conditional, a `let`,
// and a member read.
function ReadSibling() {
  useContext(C);
  return <i />;
}
export function Sibling() {
  return (
    <div>
      <C value={1}><i /></C>
      <ReadSibling />
    </div>
  );
}
function ReadTwice() {
  useContext(C);
  return <i />;
}
export function Twice() {
  return (
    <div>
      <C value={1}><ReadTwice /></C>
      <ReadTwice />
    </div>
  );
}
function MaybeWrap(props: { on: boolean; children: JSX.Element }) {
  return props.on ? <C value={1}>{props.children}</C> : props.children;
}
function ReadMaybe() {
  useContext(C);
  return <i />;
}
export function Maybe() {
  return (
    <MaybeWrap on={false}>
      <ReadMaybe />
    </MaybeWrap>
  );
}
function ReadLet() {
  useContext(L);
  return <i />;
}
export function Let() {
  return <L value={1}><ReadLet /></L>;
}
function ReadMember() {
  useContext(o.C);
  return <i />;
}
export function Member() {
  return <C value={1}><ReadMember /></C>;
}
