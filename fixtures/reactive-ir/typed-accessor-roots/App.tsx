import { createEffect, createSignal, type Accessor } from "solid-js";
import { external, makeCount, makePlain, parameterRead } from "./impl";

interface Api {
  orientation: Accessor<string>;
}
const modulePlain: Accessor<string> = () => "h";
class Model {
  orientation: Accessor<string> = () => "h";
}
declare const opaque: Accessor<string>;

// --- Silent: the value is a plain function or object the project wrote. ---
export function LiteralMember() {
  const obj: { orientation: Accessor<string> } = { orientation: () => "h" };
  const value = obj.orientation();
  return <p>{value}</p>;
}
export function InterfaceMember() {
  const obj: Api = { orientation: () => "h" };
  const value = obj.orientation();
  return <p>{value}</p>;
}
export function LocalVariable() {
  const get: Accessor<string> = () => "h";
  const value = get();
  return <p>{value}</p>;
}
export function ModuleVariable() {
  const value = modulePlain();
  return <p>{value}</p>;
}
export function PlainParameter() {
  const value = parameterRead(() => "h");
  return <p>{value}</p>;
}
export function PlainUntyped() {
  const get = () => "h";
  const value = get();
  return <p>{value}</p>;
}
export function ReturnedVariable() {
  const get = makePlain();
  const value = get();
  return <p>{value}</p>;
}
export function EffectApplyPlain() {
  const get: Accessor<number> = () => 1;
  createEffect(
    () => 1,
    () => {
      get();
    },
  );
  return <p>ok</p>;
}

// --- Violation: a source flows into the typed value. ---
export function AliasedSignal() {
  const [count] = createSignal(0);
  const get: Accessor<number> = count;
  const value = get();
  return <p>{value}</p>;
}
export function ReturnedSignal() {
  const get = makeCount();
  const value = get();
  return <p>{value}</p>;
}
export function ParameterSignal() {
  const [label] = createSignal("s");
  const value = parameterRead(label);
  return <p>{value}</p>;
}

// --- Uncertifiable: only the type says the value reads reactive state. ---
export function DeclaredValue() {
  const value = opaque();
  return <p>{value}</p>;
}
export function ClassMember() {
  const obj = new Model();
  const value = obj.orientation();
  return <p>{value}</p>;
}
export function DestructuredProp({ get }: { get: Accessor<string> }) {
  const value = get();
  return <p>{value}</p>;
}
export function DeclarationMember() {
  const value = external.orientation();
  return <p>{value}</p>;
}
// The effect-apply shortcut: a written binding has no typed entry.
export function EffectApplyWritten() {
  let get: Accessor<number> = () => 1;
  get = () => 2;
  createEffect(
    () => 1,
    () => {
      get();
    },
  );
  return <p>ok</p>;
}

// --- Review round 1 of ADR 0294. ---
// A spread before the slot: runtime slot 1 is the spread's second constant
// function, and the signal is slot 2. Syntactic element 1 is not slot 1.
function spreadFactory() {
  const [count] = createSignal(0);
  return [...([() => 0, () => 0] as const), count] as const;
}
export function SpreadTupleSlot() {
  const [, read]: readonly [unknown, Accessor<number>, ...unknown[]] = spreadFactory();
  const value = read();
  return <p>{value}</p>;
}
// No spread: element 1 is slot 1, the signal.
function slotFactory() {
  const [count] = createSignal(0);
  return [() => 0, count] as const;
}
export function ExactTupleSlot() {
  const [, read]: readonly [unknown, Accessor<number>] = slotFactory();
  const value = read();
  return <p>{value}</p>;
}
// A held arrow that reads a signal, called as a member through a helper:
// the body is visible but this path builds no summary for it.
const [tick] = createSignal(0);
const holder: { read: Accessor<number> } = { read: () => tick() };
function readHolder() {
  return holder.read();
}
export function VisibleReadingMember() {
  const value = readHolder();
  return <p>{value}</p>;
}
// The same member called in the component body: an obligation there.
export function DirectReadingMember() {
  const value = holder.read();
  return <p>{value}</p>;
}
// A held arrow proven to run nothing stays silent through a helper too.
const inertHolder: { read: Accessor<number> } = { read: () => 1 };
function readInert() {
  return inertHolder.read();
}
export function InertHeldMember() {
  const value = readInert();
  return <p>{value}</p>;
}
