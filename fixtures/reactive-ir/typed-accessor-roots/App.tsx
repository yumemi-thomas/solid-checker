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
