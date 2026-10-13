import { createMemo, createSignal, untrack } from "solid-js";
import {
  Holder,
  aliases,
  deletesA,
  destructures,
  forwards,
  guardThrows,
  keeps,
  methodCall,
  readsA,
  readsArrow,
  readsAsync,
  readsB,
  readsBranch,
  readsClosure,
  readsDefault,
  readsFn,
  readsGenerator,
  readsJsx,
  readsOrZero,
  readsReplaceable,
  readsStrict,
  readsThenB,
  sharedOptions,
  usesArguments,
  viaComputed,
  viaOptional,
  writesMember,
  writesParam,
} from "./helpers";
import * as helpers from "./helpers";
import { callsCallback } from "./helpers";
import { readsWithEval } from "./eval-helper";

declare const flag: boolean;
declare const base: { b: number };

function localReads(o: { a: number }) {
  return o.a;
}
const api = {
  read(o: { a: number }) {
    return o.a;
  },
};

// ---- Proven: the helper Gets `a` on every completion, so the getter runs
// in the component body.

export function Member() {
  const [s] = createSignal(1);
  const value = readsA({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function Arrow() {
  const [s] = createSignal(1);
  const value = readsArrow({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function SecondGet() {
  const [s] = createSignal(1);
  const value = readsThenB({ b: 1, get a() { return s(); } });
  return <main>{value}</main>;
}
export function NullishLeft() {
  const [s] = createSignal(1);
  const value = readsOrZero({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function NamedReceiver() {
  const [s] = createSignal(1);
  const options = { get a() { return s(); } };
  const value = readsA(options);
  return <main>{value}</main>;
}
export function WrappedReceiver() {
  const [s] = createSignal(1);
  const value = readsA({ get a() { return s(); } } as { a: number });
  return <main>{value}</main>;
}
export function GetterLocal() {
  const [s] = createSignal(1);
  const value = readsA({ get a() { const v = s(); return v + 1; } });
  return <main>{value}</main>;
}
export function SameFile() {
  const [s] = createSignal(1);
  const value = localReads({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function NamespaceMember() {
  const [s] = createSignal(1);
  const value = helpers.readsA({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function ObjectMethod() {
  const [s] = createSignal(1);
  const value = api.read({ get a() { return s(); } });
  return <main>{value}</main>;
}

// ---- Control, not this rule: a branch inside a callback the caller wrote
// stays under the statically-present-code premise (ADR 0275), as does a
// getter body on the contract path. This path is stricter (GetterBranch).

export function CallbackBranch() {
  const [s] = createSignal(1);
  const value = callsCallback(() => (flag ? s() : 0));
  return <main>{value}</main>;
}

// ---- No violation: the call is not in an untracked strict-read window, no
// getter runs, or another key is read.

export function Untracked() {
  const [s] = createSignal(1);
  const value = untrack(() => readsA({ get a() { return s(); } }));
  return <main>{value}</main>;
}
export function InMemo() {
  const [s] = createSignal(1);
  const value = createMemo(() => readsA({ get a() { return s(); } }));
  return <main>{value()}</main>;
}
export function DataProperty() {
  const value = readsA({ a: 1 });
  return <main>{value}</main>;
}
export function MethodValue() {
  const [s] = createSignal(1);
  const value = readsFn({ a: () => s() });
  return <main>{value()}</main>;
}
export function OtherKey() {
  const [s] = createSignal(1);
  const value = readsB({ b: 1, get a() { return s(); } });
  return <main>{value}</main>;
}

// ---- Uncertifiable, never a violation: the helper side is unproven.

export function Branch() {
  const [s] = createSignal(1);
  const value = readsBranch(flag, { get a() { return s(); } });
  return <main>{value}</main>;
}
export function Closure() {
  const [s] = createSignal(1);
  const value = readsClosure({ get a() { return s(); } });
  return <main>{value()}</main>;
}
export function Keeps() {
  const [s] = createSignal(1);
  const value = keeps({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function Aliases() {
  const [s] = createSignal(1);
  const value = aliases({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function Destructures() {
  const [s] = createSignal(1);
  const value = destructures({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function WritesParam() {
  const [s] = createSignal(1);
  const value = writesParam({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function Computed() {
  const [s] = createSignal(1);
  const value = viaComputed({ get a() { return s(); } }, "a");
  return <main>{value}</main>;
}
export function Optional() {
  const [s] = createSignal(1);
  const value = viaOptional({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function Async() {
  const [s] = createSignal(1);
  const value = readsAsync({ get a() { return s(); } });
  return <main>{String(value)}</main>;
}
export function Generator() {
  const [s] = createSignal(1);
  const value = readsGenerator({ get a() { return s(); } });
  return <main>{String(value)}</main>;
}
export function Default() {
  const [s] = createSignal(1);
  const value = readsDefault({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function Jsx() {
  const [s] = createSignal(1);
  const value = readsJsx({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function GuardThrows() {
  const [s] = createSignal(1);
  let value = 0;
  try {
    value = guardThrows({ get a() { return s(); } });
  } catch {
    value = -1;
  }
  return <main>{value}</main>;
}
export function WritesMember() {
  const [s] = createSignal(1);
  const value = writesMember({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function Deletes() {
  const [s] = createSignal(1);
  const value = deletesA({ get a() { return 0; }, get b() { return s(); } });
  return <main>{value}</main>;
}
export function Arguments() {
  const [s] = createSignal(1);
  const value = usesArguments({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function MethodCall() {
  const [s] = createSignal(1);
  const value = methodCall({ get a() { return s(); }, m() { return 1; } });
  return <main>{value}</main>;
}
export function Directive() {
  const [s] = createSignal(1);
  const value = readsStrict({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function Forwards() {
  const [s] = createSignal(1);
  const value = forwards({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function Eval() {
  const [s] = createSignal(1);
  const value = readsWithEval({ get a() { return s(); } });
  return <main>{value}</main>;
}

// ---- Uncertifiable, never a violation: the receiver is not exact.

export function Setter() {
  const [s] = createSignal(1);
  const value = readsA({ get a() { return s(); }, set a(_v: number) {} });
  return <main>{value}</main>;
}
export function SpreadInside() {
  const [s] = createSignal(1);
  const value = readsThenB({ ...base, get a() { return s(); } });
  return <main>{value}</main>;
}
export function ComputedKey() {
  const [s] = createSignal(1);
  const value = readsA({ get ["a"]() { return s(); } });
  return <main>{value}</main>;
}
export function Prototype() {
  const [s] = createSignal(1);
  const options = { __proto__: null, get a() { return s(); } };
  const value = readsA(options);
  return <main>{value}</main>;
}
export function LetReceiver() {
  const [s] = createSignal(1);
  let options = { get a() { return s(); } };
  const value = readsA(options);
  return <main>{value}</main>;
}
export function AliasedReceiver() {
  const [s] = createSignal(1);
  const options = { get a() { return s(); } };
  const copy = options;
  keeps(copy);
  const value = readsA(options);
  return <main>{value}</main>;
}
export function SharedReceiver() {
  const value = readsA(sharedOptions);
  return <main>{value}</main>;
}
export function LoopedReceiver() {
  const [s] = createSignal(1);
  const options = { get a() { return s(); } };
  let value = 0;
  for (const _ of [1]) value = readsA(options);
  return <main>{value}</main>;
}

// ---- Uncertifiable, never a violation: the getter is not proven to read.

export function GetterBranch() {
  const [s] = createSignal(1);
  const value = readsA({ get a() { return flag ? s() : 0; } });
  return <main>{value}</main>;
}
export function GetterEarlyReturn() {
  const [s] = createSignal(1);
  const value = readsA({ get a() { if (flag) return 0; return s(); } });
  return <main>{value}</main>;
}
export function GetterReceiver() {
  const [s] = createSignal(1);
  const options = { base: 1, get b() { return this.base; }, get a() { return s(); } };
  const value = readsThenB(options);
  return <main>{value}</main>;
}
export function GetterClosure() {
  const [s] = createSignal(1);
  const value = readsFn({ get a() { return () => s(); } });
  return <main>{value()}</main>;
}

// ---- Uncertifiable, never a violation: the call is not an exact mapping.

export function CallForm() {
  const [s] = createSignal(1);
  const value = readsA.call(null, { get a() { return s(); } });
  return <main>{value}</main>;
}
export function SpreadArgument() {
  const [s] = createSignal(1);
  const value = readsA(...([{ get a() { return s(); } }] as const));
  return <main>{value}</main>;
}
export function Replaceable() {
  const [s] = createSignal(1);
  const value = readsReplaceable({ get a() { return s(); } });
  return <main>{value}</main>;
}
export function Construct() {
  const [s] = createSignal(1);
  const holder = new Holder({ get a() { return s(); } });
  return <main>{holder.value}</main>;
}
