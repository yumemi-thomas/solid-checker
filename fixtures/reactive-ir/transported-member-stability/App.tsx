import { createContext, createSignal, useContext } from "solid-js";
import type { JSX } from "@solidjs/web";

function make(): {
  stable: () => number; assigned: () => number; compound: () => number;
  logical: () => number; updated: () => number; destructured: () => number;
  iterated: () => number; keyed: () => number; deleted: () => number;
  literal: () => number; defined: () => number; descriptors: () => number;
  reflected: () => number; redefined: () => number; removed: () => number;
  copied: () => number; cooked: () => number;
} {
  const [read] = createSignal(1);
  return { stable: read, assigned: read, compound: read, logical: read,
    updated: read, destructured: read, iterated: read, keyed: read, deleted: read,
    literal: read, defined: read, descriptors: read, reflected: read,
    redefined: read, removed: read, copied: read, cooked: read };
}
// Unrelated receivers deliberately veto by NAME, without guessing alias identity.
function writes() {
  const other = { assigned: () => 0, compound: 1, logical: () => 0, updated: 0,
    destructured: () => 0, iterated: () => 0, keyed: "", deleted: () => 0,
    literal: () => 0, cooked: () => 0 };
  other.assigned = () => 0;
  other.compound += 1;
  other.logical ||= () => 0;
  other.updated++;
  [other.destructured] = [() => 0];
  for (other.iterated of [() => 0]) {}
  for (other.keyed in { replaced: true }) {}
  delete (other as { deleted?: () => number }).deleted;
  other["literal"] = () => 0;
  other["co\u006fked"] = () => 0;
  Object.defineProperty(other, "defined", { value: () => 0 });
  Object.defineProperties(other, { descriptors: { value: () => 0 } });
  Reflect.set(other, "reflected", () => 0);
  Reflect.defineProperty(other, "redefined", { value: () => 0 });
  Reflect.deleteProperty(other, "removed");
  Object.assign(other, { copied: () => 0 });
}
export function VisibleVetoes() {
  const store = make();
  const value = [store.assigned(), store.compound(), store.logical(), store.updated(),
    store.destructured(), store.iterated(), store.keyed(), store.deleted(),
    store.literal(), store.defined(), store.descriptors(), store.reflected(),
    store.redefined(), store.removed(), store.copied(), store.cooked()];
  return <p>{value.join()}</p>;
}
export function StableBody() {
  const store = make();
  const value = store.stable();
  return <p>{value}</p>;
}
export function StableTracked() {
  const store = make();
  return <p>{store.stable()}</p>;
}

function nested(): { inner: { nestedRead: () => number } } {
  const [nestedRead] = createSignal(1);
  return { inner: { nestedRead } };
}
function replaceInner(inner: { nestedRead: () => number }) { inner.nestedRead = () => 0; }
export function NestedHelperMutation() {
  const store = nested();
  replaceInner(store.inner);
  const value = store.inner.nestedRead();
  return <p>{value}</p>;
}
export function NestedAliasMutation() {
  const store = nested();
  const alias = store.inner;
  alias.nestedRead = () => 0;
  const value = store.inner.nestedRead();
  return <p>{value}</p>;
}

type Account = { state: () => number };
const AccountContext = createContext<Account>();
export function AccountProvider(props: { children?: JSX.Element }) {
  const [state] = createSignal(1);
  return <AccountContext value={{ state }}>{props.children}</AccountContext>;
}
const useAccount = () => useContext(AccountContext);
function replaceAccount(read: () => Account) {
  const replaced = read();
  replaced.state = () => 0;
}
function replaceSecond() {
  const replaced = useAccount();
  replaced.state = () => 0;
}
export function WrapperCallbackMutation() {
  replaceAccount(useAccount);
  const account = useContext(AccountContext);
  const value = account.state();
  return <p>{value}</p>;
}
export function SecondConsumerMutation() {
  replaceSecond();
  const account = useAccount();
  const value = account.state();
  return <p>{value}</p>;
}

function dynamic(): { dynamicRead: () => number } {
  const [dynamicRead] = createSignal(1);
  return { dynamicRead };
}
export function ExactComputedReceiver(key: "dynamicRead") {
  const store = dynamic();
  store[key] = () => 0;
  const value = store.dynamicRead();
  return <p>{value}</p>;
}
// A computed nonliteral write on an unrelated receiver is outside the premise.
function unrelatedComputed(other: Record<string, () => number>, key: string) {
  other[key] = () => 0;
}
// Local homonyms must not be mistaken for the standard-library receiver.
function homonym() {
  const Object = { assign: (_: unknown, source: unknown) => source,
    defineProperty: (_: unknown, key: string, value: unknown) => value };
  Object.assign({}, { stable: () => 0 });
  Object.defineProperty({}, "stable", {});
  const Reflect = { set: (_: unknown, key: string, value: unknown) => value };
  Reflect.set({}, "stable", () => 0);
}
// A library signature alone does not establish the runtime receiver identity.
function libraryTypedLocal() {
  const fake: ObjectConstructor = Object;
  fake.assign({}, { stable: () => 0 });
  fake.defineProperty({}, "stable", {});
}

function wrappedMake(): {
  wrappedAs: () => number; wrappedSatisfies: () => number;
  wrappedDelete: () => number; wrappedDescriptors: () => number;
  templateKey: () => number;
} {
  const [read] = createSignal(1);
  return { wrappedAs: read, wrappedSatisfies: read, wrappedDelete: read,
    wrappedDescriptors: read, templateKey: read };
}
function wrappedWrites(read: () => ReturnType<typeof wrappedMake>) {
  const replaced = read();
  replaced["wrappedAs" as const] = () => 0;
  replaced[("wrappedSatisfies" satisfies string)] = () => 0;
  delete (replaced as { wrappedDelete?: () => number })["wrappedDelete"!];
  Object.defineProperties(replaced, {
    ["wrappedDescriptors" as const]: { value: () => 0 }
  });
  replaced[`templateKey`] = () => 0;
}
export function WrappedLiteralVetoes() {
  wrappedWrites(wrappedMake);
  const store = wrappedMake();
  const value = [store.wrappedAs(), store.wrappedSatisfies(), store.wrappedDelete(),
    store.wrappedDescriptors(), store.templateKey()];
  return <p>{value.join()}</p>;
}
// Deliberately escaped dotted names/callees are outside ADR 0273's premise.
// These unused controls pin that boundary without affecting StableBody.
function obfuscated(other: { stable: () => number }) {
  other.st\u0061ble = () => 0;
  Object.ass\u0069gn(other, { stable: () => 0 });
}
