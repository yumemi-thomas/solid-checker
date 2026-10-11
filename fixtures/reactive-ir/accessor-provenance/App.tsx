import { createEffect, createMemo, createSignal } from "solid-js";
import * as factories from "./factory";
import { collection, computed, memo, several, spread } from "./factory";
import { AccountContext, useAccount, useAmbiguous, useDb, useDefaulted } from "./context";
import { useContext } from "solid-js";

export function Body() {
  const store = collection(1);
  const a = store.rows();
  const b = store.selected();
  const c = store.plain();
  return <p>{a}{b}{c}</p>;
}
export function Eager() {
  const store = collection(1);
  const has = () => store.rows() > 0;
  const [value] = createSignal(has());
  return <p>{value()}</p>;
}
export function Tracked() {
  const store = collection(1);
  const value = createMemo(() => store.rows());
  return <p>{store.selected()}{value()}</p>;
}
export function MemoBody() {
  const store = memo();
  const value = store.value();
  return <p>{value}</p>;
}
export function NamespaceBody() {
  const store = factories.collection(1);
  const value = (store as { rows: () => number }).rows();
  return <p>{value}</p>;
}
export function AccountBody() {
  const account = useAccount();
  const value = account.user();
  return <p>{value}{account.plain()}</p>;
}
export function ContextDirect() {
  const account = useContext(AccountContext);
  const value = account.state();
  return <p>{value}</p>;
}
export function DbBody() {
  const db = useDb();
  const [value] = createSignal(db());
  return <p>{value()}</p>;
}
export function AccountApply() {
  const account = useAccount();
  const load = async () => { const value = account.user(); console.log(value); };
  createEffect(() => account.state(), () => { void load(); });
  return <p>{account.user()}</p>;
}
export function FactoryApply() {
  const store = collection(1);
  createEffect(() => store.rows(), () => { console.log(store.selected()); });
  return <p>{store.rows()}</p>;
}
export function DbApply() {
  const db = useDb();
  createEffect(() => db(), () => { console.log(db()); });
  return <p>{db()}</p>;
}
export function AccountAfterAwait() {
  const account = useAccount();
  const load = async () => { await Promise.resolve(); console.log(account.user()); };
  createEffect(() => account.state(), () => { void load(); });
  return <p>{account.user()}</p>;
}
export function EventOnly() {
  const account = useAccount();
  const store = collection(1);
  return <button onClick={() => { console.log(account.user(), store.selected()); }}>Read</button>;
}
export function Reassigned() {
  const store: { rows: () => number } = collection(1);
  store.rows = () => 0;
  const value = store.rows();
  return <p>{value}</p>;
}
export function Escaped() {
  const store = collection(1);
  unknown(store);
  const value = store.rows();
  return <p>{value}</p>;
}
declare function unknown(value: unknown): void;
export function Mutable() {
  let store: { rows: () => number } = collection(1);
  store = { rows: () => 0 };
  const value = store.rows();
  return <p>{value}</p>;
}
export function AmbiguousBody() {
  const account = useAmbiguous();
  const value = account.user();
  return <p>{value}</p>;
}
export function DefaultBody() {
  const account = useDefaulted();
  const value = account.user();
  return <p>{value}</p>;
}
export function RefusedFactories(props: { flag: boolean; key: string }) {
  const a = several(props.flag);
  const b = computed(props.key);
  const c = spread();
  const values = [a.rows(), b.rows(), c.rows()];
  return <p>{values.join()}</p>;
}

import { AttributeConsumer, ExpressionConsumer } from "./consumer";

export function PropObjectNotes() {
  const feed = collection(1);
  const notes = () => {
    const row = feed.rows();
    return <p>{row}</p>;
  };
  const decor = { notes };
  return <AttributeConsumer decor={decor} />;
}
export function PropObjectHeader() {
  const feed = collection(1);
  const decor = {
    header: () => {
      const head = feed.selected();
      return <p>{head}</p>;
    },
  };
  return <ExpressionConsumer decor={decor} />;
}
function makeDecor() {
  const feed = collection(1);
  const notes = () => {
    const row = feed.rows();
    return <p>{row}</p>;
  };
  return { notes };
}
export function ObjectConsumerBody() {
  const decor = makeDecor();
  const value = decor.notes();
  return <p>{value}</p>;
}

function retainAnimation(options: { onFrame: () => number }) {
  return () => options.onFrame();
}
export function RetainedOptionCallback() {
  const [isAnimating] = createSignal(false);
  const animate = () => isAnimating() ? 1 : 0;
  const frame = retainAnimation({ onFrame: () => animate() });
  return <button onClick={() => frame()}>Animate</button>;
}

function makeTrackedDecor() {
  const feed = collection(1);
  const notes = () => {
    const row = feed.rows();
    return <p>{row}</p>;
  };
  return { notes };
}
export function ObjectConsumerTracked() {
  const decor = makeTrackedDecor();
  return <p>{decor.notes()}</p>;
}

function nestedCollection(): { inner: { rows: () => number } } {
  const [rows] = createSignal(1);
  return { inner: { rows } };
}
function replaceInner(inner: { rows: () => number }) { inner.rows = () => 0; }
export function NestedHelperMutation() {
  const store = nestedCollection();
  replaceInner(store.inner);
  const value = store.inner.rows();
  return <p>{value}</p>;
}
export function NestedAliasMutation() {
  const store = nestedCollection();
  const inner = store.inner;
  inner.rows = () => 0;
  const value = store.inner.rows();
  return <p>{value}</p>;
}
export function NestedOpaqueHelper() {
  const store = nestedCollection();
  unknown(store.inner);
  const value = store.inner.rows();
  return <p>{value}</p>;
}
export function NestedUnmutated() {
  const store = nestedCollection();
  const value = store.inner.rows();
  return <p>{value}</p>;
}
export function MixedInstances() {
  const unproven = makeDecor();
  unknown(unproven);
  const tracked = makeDecor();
  return <p>{tracked.notes()}</p>;
}
export function MixedSameInstance() {
  const decor = makeDecor();
  unknown(decor);
  return <p>{decor.notes()}</p>;
}
export function MemberAlias() {
  const store = collection(1);
  const rows = store.rows;
  const value = rows();
  return <p>{value}</p>;
}
export function LoopMemberWrite() {
  const store = collection(1);
  for (store.rows of [() => 0]) {}
  const value = store.rows();
  return <p>{value}</p>;
}
export function LoopMemberKeyWrite() {
  const store = loopCollection();
  for (store.label in { replaced: true }) {}
  const value = store.rows();
  return <p>{value}</p>;
}
function loopCollection(): { rows: () => number; label: string } {
  const [rows] = createSignal(1);
  return { rows, label: "initial" };
}
function asyncCollection(): { ready: () => Promise<number> } {
  const [rows] = createSignal(1);
  return { ready: async () => {
    await Promise.resolve();
    return rows();
  } };
}
export function AsyncMemberPreference() {
  const store = asyncCollection();
  return <p>{(store.ready() instanceof Promise) && <span>ready</span>}</p>;
}
function retainedCondition(): { ready: () => Promise<number> } {
  const [rows] = createSignal(1);
  return { ready: () => Promise.resolve().then(() => rows()) };
}
export function RetainedMemberPreference() {
  const store = retainedCondition();
  return <p>{(store.ready() instanceof Promise) && <span>ready</span>}</p>;
}
export function SynchronousMemberPreference() {
  const store = collection(1);
  return <p>{store.rows() > 0 && <span>ready</span>}</p>;
}
export function DirectMemoPreference() {
  const value = memo();
  return <p>{value.value() > 0 && <span>ready</span>}</p>;
}
function makeRunner() {
  return { run: (read: () => number) => read() };
}
export function ParameterCallback() {
  const [count] = createSignal(1);
  const runner = makeRunner();
  const value = runner.run(count);
  return <p>{value}</p>;
}
export function ClosureParameterCallback() {
  const [count] = createSignal(1);
  const runner = makeRunner();
  const value = runner.run(() => count());
  return <p>{value}</p>;
}
function makeDeferredRunner() {
  return { run: (read: () => number) => Promise.resolve().then(read) };
}
export function DeferredParameterCallback() {
  const [count] = createSignal(1);
  const runner = makeDeferredRunner();
  const value = runner.run(count);
  return <p>{value instanceof Promise ? "waiting" : "ready"}</p>;
}

function aliasFactory(): { rows: () => number } {
  const [rows] = createSignal(1);
  const result = { rows };
  return result;
}
export function ReturnedObjectAlias() {
  const store = aliasFactory();
  const value = store.rows();
  return <p>{value}</p>;
}
export function ContextEscaped() {
  const account = useAccount();
  unknown(account);
  const value = account.state();
  return <p>{value}</p>;
}
export function ContextMemberAlias() {
  const account = useAccount();
  const state = account.state;
  const value = state();
  return <p>{value}</p>;
}
export const exportedStore = collection(1);
export function ExportedReceiver() {
  const value = exportedStore.rows();
  return <p>{value}</p>;
}
