import { createSignal } from "solid-js";
import { makeProviders } from "./factory";
export function IndexWrite() {
  const providers = makeProviders();
  providers[0] = { query: () => 0 };
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function WrappedIndex() {
  const providers = makeProviders();
  providers[(0 as const)] = { query: () => 0 };
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Delete() {
  const providers = makeProviders();
  delete providers[0];
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Length() {
  const providers = makeProviders();
  providers.length = 0;
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Push() {
  const providers = makeProviders();
  providers.push({ query: () => 0 });
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Pop() {
  const providers = makeProviders();
  providers.pop();
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Splice() {
  const providers = makeProviders();
  providers.splice(0, 1);
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Shift() {
  const providers = makeProviders();
  providers.shift();
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Unshift() {
  const providers = makeProviders();
  providers.unshift({ query: () => 0 });
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Sort() {
  const providers = makeProviders();
  providers.sort();
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Reverse() {
  const providers = makeProviders();
  providers.reverse();
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Fill() {
  const providers = makeProviders();
  providers.fill({ query: () => 0 });
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function CopyWithin() {
  const providers = makeProviders();
  providers.copyWithin(0, 1);
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Assign() {
  const providers = makeProviders();
  Object.assign(providers, { 0: { query: () => 0 } });
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function DefineProperty() {
  const providers = makeProviders();
  Object.defineProperty(providers, "0", { value: { query: () => 0 } });
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function DefineProperties() {
  const providers = makeProviders();
  Object.defineProperties(providers, { "0": { value: { query: () => 0 } } });
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function ReflectSet() {
  const providers = makeProviders();
  Reflect.set(providers, "0", { query: () => 0 });
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function ReflectDelete() {
  const providers = makeProviders();
  Reflect.deleteProperty(providers, "0");
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function ReflectDefine() {
  const providers = makeProviders();
  Reflect.defineProperty(providers, "0", { value: { query: () => 0 } });
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Spread() {
  const providers = makeProviders();
  const alias = [...providers]; void alias;
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Destructure() {
  const providers = makeProviders();
  const [first] = providers; void first;
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Storage() {
  const providers = makeProviders();
  const box = { providers }; void box;
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function Iteration() {
  const providers = makeProviders();
  for (const p of providers) { void p; }
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function LoopWrite() {
  const providers = makeProviders();
  for (providers[0] of [{query: () => 0}]) {}
  const values = providers.map(p => p.query("veto"));
  return <p>{values.join()}</p>;
}
export function DirectBody() { const [count] = createSignal(1); const value = count(); return <p>{value}</p>; }
const exported = makeProviders();
export { exported };
export function Exported() { const values = exported.map(p => p.query("export")); return <p>{values.join()}</p>; }
