import * as Solid from "solid-js";
import * as Web from "@solidjs/web";
import { createMemo, createProjection, createSignal as derivedSignal, createStore, Loading, Loading as Await, onSettled } from "solid-js";
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

export function BadLeaf() {
  onSettled(() => void user().name);
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

declare function streamUser(): AsyncIterable<{ name: string }>;
declare function maybeStreamUser(): AsyncIterable<{ name: string }> | { name: string };
