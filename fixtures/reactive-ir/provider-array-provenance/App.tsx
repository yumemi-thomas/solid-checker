import { createEffect, createMemo, createSignal } from "solid-js";
import { makeProviders, makePlain, makeAsync, makeOpen, makeAlternative, type Provider } from "./factory";

export function Body() {
  const providers = makeProviders();
  const values = providers.map(p => p.query("body"));
  return <p>{values.join()}</p>;
}
export function Apply() {
  const providers = makeProviders();
  createEffect(() => 1, () => { providers.forEach(p => p.query("apply")); });
  return <p>apply</p>;
}
export function Eager() {
  const providers = makeProviders();
  const [values] = createSignal(providers.map(p => p.query("eager")));
  return <p>{values().join()}</p>;
}
export function Tracked() {
  const providers = makeProviders();
  const values = createMemo(() => providers.map(p => p.query("tracked")));
  return <p>{values().join()}</p>;
}
export function Plain() {
  const providers = makePlain();
  const values = providers.map(p => p.query("plain"));
  return <p>{values.join()}</p>;
}
declare function escape(providers: Provider[]): void;
export function Escaped() {
  const providers = makeProviders();
  escape(providers);
  return <p>{providers.map(p => p.query("escape")).join()}</p>;
}
export function Aliased() {
  const providers = makeProviders();
  const alias = providers;
  const values = alias.map(p => p.query("alias"));
  return <p>{values.join()}</p>;
}
export function Replaced() {
  const providers = makeProviders();
  providers[0] = { query: () => 0 };
  const values = providers.map(p => p.query("replace"));
  return <p>{values.join()}</p>;
}
export function NonMap() {
  const providers = makeProviders();
  const values = providers.flatMap(p => [p.query("flat")]);
  return <p>{values.join()}</p>;
}
export function StoredCallback() {
  const providers = makeProviders();
  const callback = (p: Provider) => p.query("later");
  const values = providers.map(callback);
  callback({ query: () => 0 });
  return <p>{values.join()}</p>;
}
export function ElementAlias() {
  const providers = makeProviders();
  const values = providers.map(p => { const alias = p; return alias.query("alias"); });
  return <p>{values.join()}</p>;
}
export function AsyncMember() {
  const providers = makeAsync();
  const values = providers.map(p => p.query());
  return <p>{values.length}</p>;
}
export function AsyncCallback() {
  const providers = makeProviders();
  const values = providers.map(async p => { await Promise.resolve(); return p.query("async"); });
  return <p>{values.length}</p>;
}
export function OpenElements() {
  const providers = makeOpen();
  const values = providers.map(p => p.query("open"));
  return <p>{values.join()}</p>;
}
export function Alternative() {
  const providers = makeAlternative(true);
  const values = providers.map(p => p.query("alternative"));
  return <p>{values.join()}</p>;
}
export function HelperParameter() {
  const providers = makeProviders();
  function consume(items: Provider[]) { return items.map(p => p.query("helper")); }
  const values = consume(providers);
  return <p>{values.join()}</p>;
}

import { makeInline, makeGenerator, makeMethod, makeHoles, makeSpread } from "./factory";
export function MissingSourceNode() {
  const providers = makeInline();
  const values = providers.map(p => p.query("inline"));
  return <p>{values.join()}</p>;
}
export function GeneratorMember() {
  const providers = makeGenerator();
  const values = providers.map(p => p.query());
  return <p>{values.length}</p>;
}
export function MethodMember() {
  const providers = makeMethod();
  const values = providers.map(p => p.query());
  return <p>{values.join()}</p>;
}
export function Holes() {
  const providers = makeHoles();
  const values = providers.map(p => p!.query("hole"));
  return <p>{values.join()}</p>;
}
export function SpreadArray() {
  const providers = makeSpread();
  const values = providers.map(p => p.query("spread"));
  return <p>{values.join()}</p>;
}
export function HelperApply() {
  const providers = makeProviders();
  function query() { return providers.map(p => p.query("helper")); }
  createEffect(() => 1, () => { void query(); });
  return <p>helper apply</p>;
}
export function TrackedAndStored() {
  const providers = makeProviders();
  const callback = () => { return providers.map(p => p.query("stored")); };
  const memo = createMemo(callback);
  const box = { callback };
  declareRetain(box);
  return <p>{memo().join()}</p>;
}
declare function declareRetain(box: { callback: () => number[] }): void;
export function ConditionalCallback() {
  const providers = makeProviders();
  const values = providers.map(p => false ? p.query("never") : 0);
  return <p>{values.join()}</p>;
}
export function NeverInvokedMember() {
  const providers = makeProviders();
  const values = providers.map(p => { throw new Error("never"); return p.query("never"); });
  return <p>{values.join()}</p>;
}
import { makeTyped } from "./factory";
export function TypeOnly() {
  const providers = makeTyped();
  const values = providers.map(p => p.query("typed"));
  return <p>{values.join()}</p>;
}
export function TypeOnlyHelper() {
  const providers = makeTyped();
  function query() { return providers.map(p => p.query("typed helper")); }
  const values = query();
  return <p>{values.join()}</p>;
}
