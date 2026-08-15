import * as Solid from "solid-js";
import * as Web from "@solidjs/web";
import { createMemo, createProjection, createSignal as derivedSignal, createStore, Loading, Loading as Await, onSettled, refresh } from "solid-js";
import { dynamic } from "@solidjs/web";

const user = createMemo(async () => ({ name: "Ada" }));
const fetchedUser = createMemo(() => fetchUser());
const promisedValue = Promise.resolve({ name: "Margaret" });
const promisedUser = createMemo(() => promisedValue);
const streamedUser = createMemo(() => streamUser());
const maybeStreamedUser = createMemo(() => maybeStreamUser());
const syncUser = createMemo(() => ({ name: "Grace" }));
const [signalUser] = derivedSignal(async () => ({ name: "Lin" }));
const [storeUser] = createStore(async () => ({ name: "Edsger" }), { name: "" });
const projectedUser = Solid.createProjection(async () => ({ name: "Barbara" }), { name: "" });

export function BadDirect() {
  const name = user().name;
  return <div>{name}</div>;
}

// The same invalid execution through an async createSignal result exercises
// SC5001 independently of createMemo's result accessor.
export function BadSignalDirect() {
  const name = signalUser().name;
  return <div>{name}</div>;
}

export function BadLeaf() {
  onSettled(() => void user().name);
  return <div />;
}

// A refresh can put a previously settled source back in flight. The read
// immediately after it still occurs in onSettled's forbidden leaf owner.
export function BadRefetchInLeaf() {
  onSettled(() => {
    refresh(user);
    void user().name;
  });
  return <div />;
}

export function BadRender() {
  return <div>{user().name}{fetchedUser().name}{promisedUser().name}{streamedUser().name}{maybeStreamedUser().name}{signalUser().name}{storeUser.name}{projectedUser.name}</div>;
}

export function GoodRender() {
  return <Loading fallback={<div />}>{user().name}</Loading>;
}

export function GoodSync() {
  return <div>{syncUser().name}</div>;
}

export function GoodAliasedBoundary() {
  return <Await fallback={<div />}>{projectedUser.name}</Await>;
}

function Profile() {
  return <div>{user().name}</div>;
}

const AsyncProfile = dynamic(async () => Profile);
const AsyncNamespaceProfile = Web.dynamic(async () => Profile);
const SyncProfile = dynamic(() => Profile);

export function BadDynamicComponent() {
  return <AsyncProfile />;
}

export function GoodDynamicComponent() {
  return <Loading fallback={<div />}><AsyncProfile /></Loading>;
}

export function BadNamespaceDynamicComponent() {
  return <AsyncNamespaceProfile />;
}

export function GoodNamespaceDynamicComponent() {
  return <Loading fallback={<div />}><AsyncNamespaceProfile /></Loading>;
}

export function GoodSyncDynamicComponent() {
  return <SyncProfile />;
}

export function GoodComponentBoundary() {
  return <Loading fallback={<div />}><Profile /></Loading>;
}

export function LoadingWrapper(props: { children: JSX.Element; fallback?: JSX.Element }) {
  return <Loading fallback={props.fallback ?? <div />}>{props.children}</Loading>;
}

export function WrongLoadingWrapper(props: { children: JSX.Element; fallback?: JSX.Element }) {
  return <div>{props.children}</div>;
}

export function GoodWrapperBoundary() {
  return <LoadingWrapper fallback={<div />}>{user().name}</LoadingWrapper>;
}

export function BadWrapperBoundary() {
  return <WrongLoadingWrapper fallback={<div />}>{user().name}</WrongLoadingWrapper>;
}

// Declared first paint (rc.0): a loadingValue / seedLoadingValue node is born
// committed — its first flight never suspends readers and never trips a
// Loading boundary, so no SC5003 fires on rendering it bare. The window ends
// at the first real answer: later re-asks throw for untracked and leaf reads,
// so SC5001/SC5002 stay (with conditional wording).
const declaredFeed = createMemo(async () => ({ name: "Dorothy" }), { loadingValue: { name: "placeholder" } });
const seededUser = createProjection(async () => ({ name: "Grace" }), { name: "seed" }, { seedLoadingValue: true });
const [seededStoreUser] = createStore(async () => ({ name: "Annie" }), { name: "seed" }, { seedLoadingValue: true });

export function GoodDeclaredRender() {
  return <div>{declaredFeed().name}{seededUser.name}{seededStoreUser.name}</div>;
}

export function BadDeclaredUntracked() {
  const name = declaredFeed().name;
  return <div>{name}</div>;
}

export function BadDeclaredLeaf() {
  onSettled(() => void declaredFeed().name);
  return <div />;
}

// Fail-honest: an options argument the analyzer cannot read may declare a
// loadingValue, so the untracked-read error downgrades to uncertifiable while
// the informational boundary warning keeps firing.
declare const opaqueOptions: { loadingValue?: { name: string } };
const opaqueUser = createMemo(async () => ({ name: "Alan" }), opaqueOptions);

export function OpaqueOptionsRender() {
  return <div>{opaqueUser().name}</div>;
}

export function OpaqueOptionsUntracked() {
  const name = opaqueUser().name;
  return <div>{name}</div>;
}

declare function streamUser(): AsyncIterable<{ name: string }>;
declare function maybeStreamUser(): AsyncIterable<{ name: string }> | { name: string };
