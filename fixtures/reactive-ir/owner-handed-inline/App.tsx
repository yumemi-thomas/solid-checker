import { createContext, onCleanup, useContext } from "solid-js";
import * as S from "solid-js";

// Default-less and never provided: a read under an owner throws ContextNotFoundError.
const Ctx = createContext<number>();
// Default-less and provided above `ProvidedReader`.
const ProvidedCtx = createContext<number>();

// Positive (SC4002): a reviewed standard-library carrier runs the function before it returns, on the
// component's stack and under its owner, as `hf()` would.
export function ForEach() {
  const hf = () => {
    useContext(Ctx);
  };
  [1].forEach(hf);
  return <i>for-each</i>;
}

export function MapLiteral() {
  [1].map(() => useContext(Ctx));
  return <i>map</i>;
}

export function FromMapper() {
  const hf = () => useContext(Ctx);
  Array.from([1], hf);
  return <i>from</i>;
}

export function SortComparator() {
  const hf = (_a: number, _b: number) => {
    useContext(Ctx);
    return 0;
  };
  [2, 1].sort(hf);
  return <i>sort</i>;
}

export function ReflectApply() {
  const hf = () => {
    useContext(Ctx);
  };
  Reflect.apply(hf, null, []);
  return <i>reflect</i>;
}

// A tagged template calls its tag in place.
export function TaggedTemplate() {
  const hf = (_s: TemplateStringsArray) => {
    useContext(Ctx);
  };
  hf`x`;
  return <i>tagged</i>;
}

// Namespace: the read through the module namespace is the same read.
export function NamespaceRead() {
  const hf = () => {
    S.useContext(Ctx);
  };
  [1].forEach(hf);
  return <i>namespace</i>;
}

// Positive (SC4001): a click handler has no owner, and the carrier keeps it: NoOwnerError.
export function HandlerForEach() {
  const hf = () => {
    onCleanup(() => {});
  };
  return <button onClick={() => [1].forEach(hf)}>clean</button>;
}

// Negative: the provider supplies the context above the reader.
export function Provided() {
  return (
    <ProvidedCtx value={1}>
      <ProvidedReader />
    </ProvidedCtx>
  );
}

function ProvidedReader() {
  const hf = () => {
    useContext(ProvidedCtx);
  };
  [1].forEach(hf);
  return <i>provided</i>;
}

// Negative (obligation): a local `forEach` is not the standard library's, whatever its name; this one
// defers the callback to a timer.
function forEach(f: () => void) {
  setTimeout(f);
}
export function ShadowedForEach() {
  const hf = () => {
    useContext(Ctx);
  };
  forEach(hf);
  return <i>shadowed</i>;
}

// Negative (obligation): a project `map` method resolves to its own declaration.
class Deferred {
  map(f: () => void) {
    queueMicrotask(f);
  }
}
export function MemberMap() {
  const hf = () => {
    useContext(Ctx);
  };
  new Deferred().map(hf);
  return <i>member</i>;
}

// A fresh-stack scheduler is not an inline carrier: the callback runs with no owner (SC4001, as before).
export function Timer() {
  const hf = () => {
    useContext(Ctx);
  };
  setTimeout(hf);
  return <i>timer</i>;
}

// Negative (obligation): a generator handed to the carrier runs none of its body there.
export function GeneratorHanded() {
  function* hf() {
    useContext(Ctx);
  }
  [1].forEach(hf);
  return <i>generator</i>;
}

// Positive, as a direct call `hf()` of the same binding (ADR 0468's may-run edge): the binding may hold another
// function when the carrier runs it, and whenever the declared one runs there it runs under the component's owner.
export function Reassigned() {
  let hf = () => {
    useContext(Ctx);
  };
  if (Math.random() > 2) hf = () => {};
  [1].forEach(hf);
  return <i>reassigned</i>;
}
