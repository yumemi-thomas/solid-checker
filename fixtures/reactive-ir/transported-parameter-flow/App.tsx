import { createContext, createEffect, createMemo, createSignal, useContext } from "solid-js";
type Model = { read: () => number };
type Provider = { query: (input: string) => number };
function makeModel(): Model {
  const [read] = createSignal(1);
  return { read };
}
function makeProviders(): Provider[] {
  const [count] = createSignal(1);
  const query = (_input: string) => { return count(); };
  return [{ query }];
}

function bodyQuery(providers: Provider[]) {
  return providers.map(p => p.query("body"));
}
export function Body() {
  const providers = makeProviders();
  const values = bodyQuery(providers);
  return <p>{values.join()}</p>;
}
function applyQuery(providers: Provider[], input: string) {
  return providers.map(p => p.query(input));
}
export function Apply() {
  const providers = makeProviders();
  createEffect(() => "input", input => { void applyQuery(providers, input); });
  return <p>apply</p>;
}
async function prefixQuery(providers: Provider[], input: string) {
  const values = providers.map(p => p.query(input));
  return await Promise.all(values);
}
export function AsyncPrefix() {
  const providers = makeProviders();
  createEffect(() => "input", input => { void prefixQuery(providers, input); });
  return <p>prefix</p>;
}
function trackedQuery(providers: Provider[]) {
  return providers.map(p => p.query("tracked"));
}
export function Tracked() {
  const providers = makeProviders();
  const values = createMemo(() => trackedQuery(providers));
  return <p>{values().join()}</p>;
}

function captureModel(model: Model): Provider[] {
  const query = (_input: string) => { return model.read(); };
  return [{ query }];
}
function capturedQuery(providers: Provider[]) {
  return providers.map(p => p.query("captured"));
}
export function CapturedModel() {
  const model = makeModel();
  const providers = captureModel(model);
  createEffect(() => 1, () => { void capturedQuery(providers); });
  return <p>captured</p>;
}
const Context = createContext<Model>();
function mountModel(model: Model) {
  return <Context value={model}><ContextBody /></Context>;
}
export function Root() { const model = makeModel(); return mountModel(model); }
function useModel() { const value = useContext(Context); if (!value) throw new Error("missing"); return value; }
function ContextBody() { const model = useModel(); const n = model.read(); return <p>{n}</p>; }

function secondQuery(providers: Provider[]) { return providers.map(p => p.query("second")); }
export function SecondCall() {
  const providers = makeProviders();
  const values = secondQuery(providers);
  void secondQuery(providers);
  return <p>{values.join()}</p>;
}
export function exportedQuery(providers: Provider[]) { return providers.map(p => p.query("exported")); }
export function Exported() { const providers = makeProviders(); const values = exportedQuery(providers); return <p>{values.join()}</p>; }
function reassignedQuery(providers: Provider[]) { providers = []; return providers.map(p => p.query("reassigned")); }
export function Reassigned() { const providers = makeProviders(); const values = reassignedQuery(providers); return <p>{values.join()}</p>; }
function defaultQuery(providers: Provider[] = []) { return providers.map(p => p.query("default")); }
export function DefaultParameter() { const providers = makeProviders(); const values = defaultQuery(providers); return <p>{values.join()}</p>; }
function spreadQuery(providers: Provider[]) { return providers.map(p => p.query("spread")); }
export function SpreadArgument() { const providers = makeProviders(); const args: [Provider[]] = [providers]; const values = spreadQuery(...args); return <p>{values.join()}</p>; }
function argumentsQuery(providers: Provider[]) { void arguments; return providers.map(p => p.query("arguments")); }
export function ArgumentsUse() { const providers = makeProviders(); const values = argumentsQuery(providers); return <p>{values.join()}</p>; }
declare function retainFunction(fn: (providers: Provider[]) => number[]): void;
function escapedQuery(providers: Provider[]) { return providers.map(p => p.query("escaped")); }
export function EscapedFunction() { const providers = makeProviders(); retainFunction(escapedQuery); const values = escapedQuery(providers); return <p>{values.join()}</p>; }
function aliasedQuery(providers: Provider[]) { return providers.map(p => p.query("alias")); }
export function AliasedFunction() { const providers = makeProviders(); const alias = aliasedQuery; const values = alias(providers); return <p>{values.join()}</p>; }
declare function retainProviders(providers: Provider[]): void;
function retainingQuery(providers: Provider[]) { retainProviders(providers); return providers.map(p => p.query("retained")); }
export function EscapedParameter() { const providers = makeProviders(); const values = retainingQuery(providers); void providers.map(p => p.query("other")); return <p>{values.join()}</p>; }
async function lateQuery(providers: Provider[]) { await Promise.resolve(); return providers.map(p => p.query("late")); }
export function AfterAwait() { const providers = makeProviders(); createEffect(() => 1, () => { void lateQuery(providers); }); return <p>late</p>; }

async function awaitOperandQuery(providers: Provider[]) { return await Promise.all(providers.map(p => p.query("operand"))); }
export function AwaitOperand() { const providers = makeProviders(); createEffect(() => 1, () => { void awaitOperandQuery(providers); }); return <p>operand</p>; }

function restQuery(providers: Provider[], ..._rest: string[]) { return providers.map(p => p.query("rest")); }
export function RestParameter() { const providers = makeProviders(); const values = restQuery(providers); return <p>{values.join()}</p>; }
function preferenceQuery(providers: Provider[]) { return providers.map(p => p.query("preference")); }
export function TransportedPreference() { const providers = makeProviders(); return <p>{preferenceQuery(providers).length && <span>ready</span>}</p>; }
