import { createEffect, createSignal, createStore, untrack } from "solid-js";
import {
  useLocation, useCount, exportedAlias, arrowLocation, sameSource,
  exhaustiveSource, wrappedSource, differentSources, otherReturn,
  implicitUndefined, explicitUndefined, asyncSource, generatorSource,
  writtenSource, passedSource, escapedSource, aliasedSource, memberSource,
  finallyOverrides, loopSource, capturedSource, typedPlain, unresolvedHook,
  reassignedHook,
} from "./hook";
import * as Hooks from "./hook";
import type { LocationShape } from "./hook";

function localLocation() {
  const [location] = createStore({ search: "?plan=pro" });
  return location;
}
export function LocalStore() {
  const [location] = createStore({ search: "?plan=pro" });
  const [config] = createSignal(location.search);
  return <p>{config()}</p>;
}
export function SameFile() {
  const location = localLocation();
  const [config] = createSignal(location.search);
  return <p>{config()}</p>;
}
export function CrossFile() {
  const location = useLocation();
  const [config] = createSignal(location.search);
  return <p>{config()}</p>;
}
export function NamespaceImport() {
  const location = Hooks.useLocation();
  const [config] = createSignal(location.search);
  return <p>{config()}</p>;
}
export function ExportAlias() {
  const location = exportedAlias();
  const [config] = createSignal(location.search);
  return <p>{config()}</p>;
}
export function ArrowAndPrimitiveNamespace() {
  const location = arrowLocation();
  const [config] = createSignal(location.search);
  return <p>{config()}</p>;
}
export function SameIdentity() {
  const state = sameSource(true);
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function ExhaustiveIdentity() {
  const state = exhaustiveSource(true);
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function TransparentReturn() {
  const state = wrappedSource();
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function ReturnedCount() {
  const count = useCount();
  const [config] = createSignal(count());
  return <p>{config()}</p>;
}
export function FreshCalls() {
  const first = useLocation();
  const second = useLocation();
  const [a] = createSignal(first.search);
  const [b] = createSignal(second.search);
  return <p>{a()}{b()}</p>;
}
export function Sampled() {
  const state = useLocation();
  const [config] = createSignal(untrack(() => state.search));
  return <p>{config()}</p>;
}
export function Tracked() {
  const state = useLocation();
  createEffect(() => state.search, value => { console.log(value); });
  return <p>{state.search}</p>;
}
export function DifferentIdentity() {
  const state = differentSources(true);
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function PlainAlternative() {
  const state = otherReturn(true);
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function Fallthrough() {
  const state = implicitUndefined(true);
  if (!state) return <p>absent</p>;
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function BareReturn() {
  const state = explicitUndefined(true);
  if (!state) return <p>absent</p>;
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function AsyncReturn() {
  const pending = asyncSource();
  pending.then(state => { console.log(state.search); });
  return <p>pending</p>;
}
export function GeneratorReturn() {
  const step = generatorSource().next();
  if (!step.done) return <p>yielded</p>;
  const [config] = createSignal(step.value.search);
  return <p>{config()}</p>;
}
export function WrittenBinding() {
  const state = writtenSource();
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function PassedThroughCall() {
  const state = passedSource();
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function EscapedRoot() {
  const state = escapedSource();
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function EscapedAlias() {
  const state = aliasedSource();
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function ReturnedMember() {
  const items = memberSource();
  const [config] = createSignal(items.length);
  return <p>{config()}</p>;
}
export function FinallyReplacement() {
  const state = finallyOverrides();
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function UnmodeledLoop() {
  const state = loopSource(true);
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function CapturedNotFresh() {
  const state = capturedSource();
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function PlainAndUnresolved() {
  const plain = typedPlain();
  const unresolved = unresolvedHook();
  const [config] = createSignal(plain.search + unresolved.search);
  return <p>{config()}</p>;
}
export function ShadowedHook() {
  const useLocation = (): LocationShape => ({ search: "plain", items: [] });
  const state = useLocation();
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function ShadowedNamespace() {
  const Hooks = {
    useLocation: (): LocationShape => ({ search: "plain", items: [] }),
  };
  const state = Hooks.useLocation();
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function ShadowedPrimitive() {
  const createStore = (value: LocationShape): [LocationShape] => [value];
  function factory() {
    const [state] = createStore({ search: "plain", items: [] });
    return state;
  }
  const state = factory();
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function ReassignedFunction() {
  const state = reassignedHook();
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function ReassignedLocalFunction() {
  let factory = () => {
    const [state] = createStore({ search: "live", items: [] as string[] });
    return state;
  };
  factory = () => ({ search: "plain", items: [] });
  const state = factory();
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function WrittenCaller() {
  let state = useLocation();
  state = { search: "plain", items: [] };
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function OptionalCaller() {
  const factory: (() => LocationShape) | undefined =
    Date.now() % 2 ? useLocation : undefined;
  const state = factory?.();
  if (!state) return <p>absent</p>;
  const [config] = createSignal(state.search);
  return <p>{config()}</p>;
}
export function ConditionalReturnConsumer() {
  const state = useLocation();
  if (state.search) return <p>yes</p>;
  return <p>no</p>;
}
export function ShowPreference() {
  const state = useLocation();
  return <p>{state.search && <p>yes</p>}</p>;
}
export function ForPreference() {
  const state = useLocation();
  return <p>{state.items.map(item => <p>{item}</p>)}</p>;
}

export default function App() {
  return <main>
    <LocalStore /><SameFile /><CrossFile /><NamespaceImport /><ExportAlias />
    <ArrowAndPrimitiveNamespace /><SameIdentity /><ExhaustiveIdentity />
    <TransparentReturn /><ReturnedCount /><FreshCalls /><Sampled /><Tracked />
    <DifferentIdentity /><PlainAlternative /><Fallthrough /><BareReturn />
    <AsyncReturn /><GeneratorReturn /><WrittenBinding /><PassedThroughCall />
    <EscapedRoot /><EscapedAlias /><ReturnedMember /><FinallyReplacement />
    <UnmodeledLoop /><CapturedNotFresh /><PlainAndUnresolved /><ShadowedHook />
    <ShadowedNamespace /><ShadowedPrimitive /><ReassignedFunction /><ReassignedLocalFunction />
    <WrittenCaller /><OptionalCaller /><ConditionalReturnConsumer />
    <ShowPreference /><ForPreference />
  </main>;
}
