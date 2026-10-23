import { createSignal, type Accessor } from "solid-js";

function readsA(o: { a: string }) {
  return o.a;
}

// CERTIFIED: the getter calls `Function.prototype.toString` on the accessor,
// never `s` (before: SC1001 through the callee's leading token `s`).
export function GetterMemberCallee() {
  const [s] = createSignal(1);
  const value = readsA({ get a() { return s.toString(); } });
  return <main>{value}</main>;
}
// VIOLATION: the getter calls `s` while the body runs `readsA`.
export function GetterAccessorCall() {
  const [s] = createSignal(1);
  const value = readsA({ get a() { return String(s()); } });
  return <main>{value}</main>;
}

interface Api {
  register(source: Accessor<number>): void;
}
declare const apiA: Api;
declare const apiB: Api;
declare const flag: boolean;
declare class Registry {
  register(source: Accessor<number>): void;
}

// UNCERTIFIABLE (SC9011): the callee's leading token is `(` or `new`, which
// names no symbol; that absence dropped the obligation (before: certified).
export function ParenLedCallee() {
  const [count] = createSignal(0);
  (flag ? apiA : apiB).register(count);
  return <main />;
}
export function NewLedCallee() {
  const [count] = createSignal(0);
  new Registry().register(count);
  return <main />;
}
// UNCERTIFIABLE (SC9011): the identifier-led control.
export function IdentifierLedCallee() {
  const [count] = createSignal(0);
  apiA.register(count);
  return <main />;
}

const [tick] = createSignal(0);
class Swap {
  constructor() {
    return { bump: () => tick() } as any;
  }
}
class Holder {
  static Base = Swap;
}
// UNCERTIFIABLE (SC9012): `extends Holder.Base` is `Swap`, whose constructor
// returns another object; its leading token named `Holder`, whose instances
// are exact, so `ViaMember.bump` was selected (before: certified).
class ViaMember extends Holder.Base {
  bump(): number {
    return 1;
  }
}
// UNCERTIFIABLE (SC9012): the identifier-based control.
class ViaIdentifier extends Swap {
  bump(): number {
    return 1;
  }
}
function bumpMember(counter: ViaMember): number {
  return counter.bump();
}
function bumpIdentifier(counter: ViaIdentifier): number {
  return counter.bump();
}
export function MemberHeritage() {
  const value = bumpMember(new ViaMember());
  return <div>{value}</div>;
}
export function IdentifierHeritage() {
  const value = bumpIdentifier(new ViaIdentifier());
  return <div>{value}</div>;
}
