import { createMemo, createSignal, untrack } from "solid-js";
import makeDefault, {
  makeExpression,
  makeForward,
  makeGuarded,
  makeHoled,
  makeInline,
  makeLoop,
  makePeek,
  makeQuiet,
  makeReplaceable,
  makeShared,
  makeSpread,
  makeSpreadAccessor,
  makeStored,
  modulePeek,
} from "./factories";

declare const flag: boolean;
const fallback = () => {};

// --- Proven: the slot closure's read runs in the component body. ---

export function Loop() {
  const [running, start, stop] = makeLoop();
  start();
  return <button onClick={stop}>{String(running())}</button>;
}

export function LeadingHole() {
  const [, start] = makeLoop();
  start();
  return <main />;
}

export function DefaultImport() {
  const [, start] = makeDefault();
  start();
  return <main />;
}

export function InlineSlot() {
  const [, next] = makeInline();
  next();
  return <main />;
}

const [local] = createSignal(0);
function makeLocal() {
  const peek = () => local();
  return [local, peek] as const;
}

export function SameFile() {
  const [, peek] = makeLocal();
  peek();
  return <main />;
}

// --- Clean: the read is tracked, untracked on purpose, or absent. ---

export function InJsx() {
  const [, peek] = makePeek();
  return <main>{peek()}</main>;
}

export function InMemo() {
  const [, peek] = makePeek();
  const total = createMemo(() => peek());
  return <main>{total()}</main>;
}

export function Untracked() {
  const [, peek] = makePeek();
  untrack(() => peek());
  return <main />;
}

export function NoRead() {
  const [, ping] = makeQuiet();
  ping();
  return <main />;
}

export function OnlyAccessor() {
  const [running] = makeLoop();
  return <main>{String(running())}</main>;
}

// --- Refused: no slot identity is proven, so nothing is claimed. ---

export function Guarded() {
  const [, start] = makeGuarded();
  start();
  return <main />;
}

export function MutablePattern() {
  let [, peek] = makePeek();
  peek();
  return <main />;
}

export function DefaultedSlot() {
  const [, peek = fallback] = makePeek();
  peek();
  return <main />;
}

export function RestPattern() {
  const [first, peek, ...rest] = makeLoop();
  peek();
  return <main>{String(first())}{rest.length}</main>;
}

export function NestedPattern() {
  const [{ length }, peek] = makePeek();
  peek();
  return <main>{length}</main>;
}

export function HoledLiteral() {
  const [, , peek] = makeHoled();
  peek();
  return <main />;
}

export function SpreadLiteral() {
  const [, peek] = makeSpread();
  peek();
  return <main />;
}

export function AliasedTuple() {
  const tuple = makePeek();
  const [, peek] = tuple;
  peek();
  return <main />;
}

export function IndexedTuple() {
  const tuple = makePeek();
  tuple[1]();
  return <main />;
}

export function StoredLiteral() {
  const [, peek] = makeStored();
  peek();
  return <main />;
}

export function ExpressionSlot() {
  const [, peek] = makeExpression();
  peek();
  return <main />;
}

export function SharedSlot() {
  const [, peek] = makeShared();
  peek();
  return <main />;
}

export function ReplaceableSlot() {
  const [, peek] = makeReplaceable();
  peek();
  return <main />;
}

export function Forwarded() {
  const [, peek] = makeForward();
  peek();
  return <main />;
}

export function Branched() {
  const [, peek] = flag ? makePeek() : makeQuiet();
  peek();
  return <main />;
}

export function AccessorSlot() {
  const [running] = makeLoop();
  running();
  return <main />;
}

export function ImportedSlot() {
  modulePeek();
  return <main />;
}

export function SpreadAccessor() {
  const [, plain] = makeSpreadAccessor();
  plain();
  return <main />;
}
