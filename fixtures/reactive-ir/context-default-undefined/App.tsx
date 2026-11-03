import { createContext, createContext as makeContext, useContext } from "solid-js";
import * as S from "solid-js";

// An `undefined` default is no default (rc.17 `getContext`): a read with no provider throws ContextNotFoundError.
const UndefinedDefault = createContext<number | undefined>(undefined);
const WrappedUndefined = createContext<number | undefined>(undefined as number | undefined);
const NamespaceUndefined = S.createContext<number | undefined>(undefined);
const AliasUndefined = makeContext<number | undefined>(undefined, { name: "alias" });
// A provider of `undefined` stores the (absent) default: no provider.
const ProvidesUndefined = createContext<number | undefined>();
// Defaults proven not `undefined`: a read returns them.
const NullDefault = createContext<number | null>(null);
const LiteralDefault = createContext<number>(0);
const seed = { n: 1 };
const ConstDefault = createContext<{ n: number }>(seed);
// A default the syntax does not prove is not `undefined`.
declare function maybeDefault(): number | undefined;
const UnprovenDefault = createContext<number | undefined>(maybeDefault());
// Provided above its reader, with a value.
const ProvidedUndefinedDefault = createContext<number | undefined>(undefined);

// Positive (SC4002): no provider, `undefined` default.
export function BodyRead() {
  useContext(UndefinedDefault);
  return <div></div>;
}

// Positive: through an assertion.
export function WrappedRead() {
  useContext(WrappedUndefined);
  return <div></div>;
}

// Positive: created and read through the namespace.
export function NamespaceRead() {
  S.useContext(NamespaceUndefined);
  return <div></div>;
}

// Positive: an aliased createContext with a second argument.
export function AliasRead() {
  useContext(AliasUndefined);
  return <div></div>;
}

// Positive: a read in a render expression.
export function JsxRead() {
  return <div>{useContext(UndefinedDefault)}</div>;
}

// Positive: the only provider passes `undefined`.
export function ProviderOfUndefined() {
  return (
    <ProvidesUndefined value={undefined}>
      <ProvidesUndefinedReader />
    </ProvidesUndefined>
  );
}
function ProvidesUndefinedReader() {
  useContext(ProvidesUndefined);
  return <p>x</p>;
}

// Negative: `null`, a literal and a `const` holding an object are defaults.
export function DefaultReads() {
  useContext(NullDefault);
  useContext(LiteralDefault);
  useContext(ConstDefault);
  return <div></div>;
}

// Negative: a provider with a value above the reader.
export function Provided() {
  return (
    <ProvidedUndefinedDefault value={1}>
      <ProvidedReader />
    </ProvidedUndefinedDefault>
  );
}
function ProvidedReader() {
  useContext(ProvidedUndefinedDefault);
  return <p>x</p>;
}

// Obligation (SC4002 uncertifiable): the default may be `undefined`.
export function UnprovenRead() {
  useContext(UnprovenDefault);
  return <div></div>;
}
